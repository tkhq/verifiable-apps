//! Service to mock out QOS proxying requests to an enclave app

use borsh::BorshDeserialize;
use qos_core::{
    client::Client,
    io::{SocketAddress, TimeVal, TimeValLike},
    protocol::msg::ProtocolMsg,
    server::{RequestProcessor, SocketServer},
};
use qos_nsm::types::NsmResponse;
use tokio::task::JoinHandle;

/// Configuration for QOS simulator.
pub struct QosSimulatorConfig {
    /// Unix socket path the QOS simulator listens on.
    pub enclave_sock: String,
    /// Unix socket path the enclave app to proxy too is expected to be
    /// listening on.
    pub app_sock: String,
}

/// Spawn a QOS simulator. This will simulate QOS proxying requests from the host to application binary.
pub async fn spawn_qos_simulator(
    QosSimulatorConfig {
        enclave_sock,
        app_sock,
    }: QosSimulatorConfig,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let enclave_sock_addr = SocketAddress::new_unix(&enclave_sock);

        let app_sock_addr = SocketAddress::new_unix(&app_sock);
        let processor = Processor {
            app_client: Client::new(app_sock_addr, TimeVal::seconds(1)),
        };
        SocketServer::listen(enclave_sock_addr, processor).unwrap();
    })
}

struct Processor {
    app_client: Client,
}

impl RequestProcessor for Processor {
    fn process(&mut self, request: Vec<u8>) -> Vec<u8> {
        let msg_req = ProtocolMsg::try_from_slice(&request)
            .expect("enclave_stub: Failed to deserialize request");

        match msg_req {
            ProtocolMsg::ProxyRequest { data } => {
                let resp_data = self.app_client.send(&data).expect("Client error");

                borsh::to_vec(&ProtocolMsg::ProxyResponse { data: resp_data })
                    .expect("enclave_stub: Failed to serialize response")
            }
            ProtocolMsg::LiveAttestationDocRequest => {
                let data_string = borsh::to_vec(&"MOCK_DOCUMENT".to_string())
                    .expect("unable to serialize mock document");
                let nsm_response = NsmResponse::Attestation {
                    document: data_string,
                };

                borsh::to_vec(&ProtocolMsg::LiveAttestationDocResponse {
                    nsm_response,
                    manifest_envelope: None,
                })
                .expect("enclave stub: Failed to serialize response")
            }
            other => panic!("enclave_stub: Unexpected request {other:?}"),
        }
    }
}
