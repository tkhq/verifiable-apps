//! Utils for e2e tests. See `/tests` for e2e tests.
//! One-file integration test for the reshard stack (simulator_enclave + reshard_app + reshard_host).
use futures::FutureExt;
use std::{fs, path::{Path, PathBuf}, process::Command};

use borsh::to_vec as borsh_to_vec;

use reshard_host::generated::reshard::reshard_service_client::ReshardServiceClient;

use qos_core::protocol::services::boot::{Manifest, ManifestEnvelope};
use tempdir::TempDir;
use tonic::transport::Channel;

pub mod qos_simulator;

/// Local host IP address.
pub const LOCAL_HOST: &str = "127.0.0.1";
/// Max gRPC message size (25MB).
pub const GRPC_MAX_RECV_MSG_SIZE: usize = 26_214_400;

/// Arguments passed to the user test callback.
pub struct TestArgs {
    /// Reshard gRPC client.
    pub reshard_client: ReshardServiceClient<Channel>,
}

/// Kills a child process on drop.
#[derive(Debug)]
pub struct ChildWrapper(std::process::Child);
impl From<std::process::Child> for ChildWrapper {
    fn from(child: std::process::Child) -> Self {
        Self(child)
    }
}
impl Drop for ChildWrapper {
    fn drop(&mut self) {
        let _ = self.0.kill();
    }
}

/// Bring up the stack, run `test`, then tear down.
pub async fn execute<F, T>(test: F)
where
    F: Fn(TestArgs) -> T,
    T: std::future::Future<Output = ()>,
{
    let tmp_dir = TempDir::new("testharness").unwrap();

    // Socket paths
    let app_sock = tmp_dir.path().join(".reshard.app.sock");
    let enc_sock = tmp_dir.path().join(".reshard.enclave.sock");

    // Minimal manifest envelope
    let manifest_path = tmp_dir.path().join(".manifest_envelope");
    write_minimal_manifest(&manifest_path);

    // 1) simulator_enclave
    let _join_handle = qos_simulator::spawn_qos_simulator(qos_simulator::QosSimulatorConfig {
        enclave_sock: enc_sock.to_str().unwrap().to_string(),
        app_sock: app_sock.to_str().unwrap().to_string(),
    });

    // 2) reshard_app
    let new_share_dir = Path::new("./fixtures/reshard/new-share-set");
    let threshold = load_threshold(&new_share_dir.join("quorum_threshold"));
    let members = joined_pubkeys(new_share_dir);
    let quorum_secret = "./fixtures/reshard/quorum.secret";
    let ephemeral_secret = "./fixtures/reshard/ephemeral.secret";
    let _app: ChildWrapper = Command::new("../target/debug/reshard_app")
        .arg("--usock")
        .arg(&app_sock)
        .arg("--quorum-file")
        .arg(quorum_secret)
        .arg("--ephemeral-file")
        .arg(ephemeral_secret)
        .arg("--manifest-file")
        .arg(&manifest_path)
        .arg("--threshold")
        .arg(&threshold)
        .arg("--members")
        .arg(&members)
        .arg("--mock-nsm")
        .spawn()
        .expect("spawn reshard_app")
        .into();

    // 3) reshard_host
    let host_port = qos_test_primitives::find_free_port().expect("find free port");
    let _host: ChildWrapper = Command::new("../target/debug/reshard_host")
        .arg("--host-ip")
        .arg(LOCAL_HOST)
        .arg("--host-port")
        .arg(host_port.to_string())
        .arg("--usock")
        .arg(&enc_sock)
        .spawn()
        .expect("spawn reshard_host")
        .into();
    qos_test_primitives::wait_until_port_is_bound(host_port);

    let host_addr = format!("http://{LOCAL_HOST}:{host_port}");

    let reshard = ReshardServiceClient::connect(host_addr)
        .await
        .unwrap()
        .max_decoding_message_size(GRPC_MAX_RECV_MSG_SIZE);

    let test_args = TestArgs {
        reshard_client: reshard,
    };

    // Run the user test and ensure cleanup.
    let res = std::panic::AssertUnwindSafe(test(test_args))
        .catch_unwind()
        .await;
    assert!(res.is_ok(), "test body panicked");
}

fn write_minimal_manifest(path: &PathBuf) {
    let env = ManifestEnvelope {
        manifest: Manifest {
            ..Default::default()
        },
        ..Default::default()
    };
    let bytes = borsh_to_vec(&env).expect("borsh ManifestEnvelope");
    fs::write(path, bytes).expect("write manifest");
}

fn load_threshold(th_path: &Path) -> String {
    fs::read_to_string(th_path)
        .expect("read quorum_threshold")
        .trim()
        .to_string()
}

fn joined_pubkeys(dir: &Path) -> String {
    // collect *.pub files
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("failed to read dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("pub"))
        .collect();

    // deterministic order
    files.sort();

    // read, strip whitespace, join with ';'
    let keys: Vec<String> = files.into_iter()
        .map(|p| fs::read_to_string(&p).expect("read .pub"))
        .map(|s| s.split_whitespace().collect::<String>())
        .filter(|s| !s.is_empty())
        .collect();

    if keys.is_empty() {
        panic!("no *.pub files found in {}", dir.display());
    }

    keys.join(";")
}
