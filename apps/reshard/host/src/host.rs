use std::sync::Arc;

use crate::generated::{
    reshard::reshard_service_server::{ReshardService, ReshardServiceServer},
    reshard::{RetrieveReshardRequest, RetrieveReshardResponse},
    FILE_DESCRIPTOR_SET,
};
use health_check::{spawn_k8s_health_checker, AppHealthCheckable, AppHealthResponse};
use host_primitives::{spawn_queue_consumer, wait_for_sigterm, BorshCodec};
use host_primitives::{EnclaveClient, GRPC_MAX_RECV_MSG_SIZE};
use qos_core::io::SocketAddress;
use reshard_app::service::{ReshardRequest, ReshardResponse};
use tokio::sync::{mpsc, oneshot};
use tonic::Status;

type EnclaveQueueMsg = host_primitives::EnclaveQueueMsg<ReshardRequest, ReshardResponse>;

/// Start the host server.
pub async fn listen(
    listen_addr: std::net::SocketAddr,
    enclave_addr: SocketAddress,
) -> Result<(), tonic::transport::Error> {
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("failed to start reflection service");

    let (queue_tx, queue_rx) =
        mpsc::channel::<Box<EnclaveQueueMsg>>(host_primitives::ENCLAVE_QUEUE_CAPACITY);
    let enclave = Arc::new(EnclaveClient::new(queue_tx));

    let app_checker = Health {
        enclave: enclave.clone(),
    };
    let health_service = spawn_k8s_health_checker(Arc::new(app_checker)).await;

    let host: Host = Host {
        enclave: enclave.clone(),
    };
    spawn_queue_consumer::<BorshCodec, _, _>(enclave_addr, queue_rx);

    println!("HostServer listening on {listen_addr}");

    let (sigterm_sender, sigterm_receiver) = oneshot::channel();
    tokio::task::spawn(wait_for_sigterm(sigterm_sender));

    tonic::transport::Server::builder()
        .add_service(reflection_service)
        .add_service(health_service)
        .add_service(
            ReshardServiceServer::new(host).max_decoding_message_size(GRPC_MAX_RECV_MSG_SIZE),
        )
        .serve_with_shutdown(listen_addr, async {
            sigterm_receiver.await.ok();
            println!("SIGTERM received");
        })
        .await
}

/// Host `gRPC` server.
#[derive(Debug)]
pub struct Host {
    /// Sender for enclave queue. Enclave queue is for messages waiting to be sent to the enclave.
    enclave: Arc<EnclaveClient<BorshCodec, ReshardRequest, ReshardResponse>>,
}

#[tonic::async_trait]
impl ReshardService for Host {
    async fn retrieve_reshard(
        &self,
        _: tonic::Request<RetrieveReshardRequest>,
    ) -> std::result::Result<tonic::Response<RetrieveReshardResponse>, Status> {
        let app_response = self.enclave.send(ReshardRequest::RetrieveBundle).await?;

        let ReshardResponse::Bundle(bundle) = app_response else {
            return Err(Status::internal("received invalid response from app"));
        };

        let reshard_bundle = serde_json::to_string(&*bundle)
            .map_err(|_| Status::internal("received invalid json bundle from app"))?;
        let response = RetrieveReshardResponse { reshard_bundle };
        Ok(tonic::Response::new(response))
    }
}

struct Health {
    enclave: Arc<EnclaveClient<BorshCodec, ReshardRequest, ReshardResponse>>,
}

#[tonic::async_trait]
impl AppHealthCheckable for Health {
    async fn app_health_check(&self) -> Result<tonic::Response<AppHealthResponse>, tonic::Status> {
        let app_response = self.enclave.send(ReshardRequest::HealthRequest).await?;
        if ReshardResponse::Health != app_response {
            return Err(Status::internal("received invalid response from app"));
        }

        Ok(tonic::Response::new(AppHealthResponse { code: 200 }))
    }
}
