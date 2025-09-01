use reshard_app::service::{ReshardRequest, ReshardResponse};
use tokio::sync::mpsc;

type EnclaveQueueMsg = host_primitives::EnclaveQueueMsg<ReshardRequest, ReshardResponse>;


/// Host `gRPC` server.
#[derive(Debug)]
pub struct Host {
    /// Sender for enclave queue. Enclave queue is for messages waiting to be sent to the enclave.
    queue_tx: mpsc::Sender<Box<EnclaveQueueMsg>>,
}

impl Host {
    fn new(queue_tx: mpsc::Sender<Box<EnclaveQueueMsg>>) -> Self {
        Self { queue_tx }
    }

    /// Start the host server.
    pub async fn listen(
        listen_addr: std::net::SocketAddr,
        enclave_addr: SocketAddress,
    ) -> Result<(), tonic::transport::Error> {
        let reflection_service = gen::tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(gen::FILE_DESCRIPTOR_SET)
            .build()
            .expect("failed to start reflection service");

        let (queue_tx, queue_rx) = mpsc::channel::<Box<EnclaveQueueMsg>>(host_primitives::ENCLAVE_QUEUE_CAPACITY);

        let app_checker = Health {
            queue_tx: queue_tx.clone(),
        };


        let host = Host::new(queue_tx);
        spawn_queue_consumer(enclave_addr, queue_rx);

        println!("HostServer listening on {listen_addr}");

        let (sigterm_sender, sigterm_receiver) = oneshot::channel();
        tokio::task::spawn(wait_for_sigterm(sigterm_sender));

        tonic::transport::Server::builder()
            .add_service(reflection_service)
            .add_service(
                reshard_service_server::ReshardServiceServer::new(host).max_decoding_message_size(GRPC_MAX_RECV_MSG_SIZE),
            )
            .serve_with_shutdown(listen_addr, async {
                sigterm_receiver.await.ok();
                println!("SIGTERM received");
            })
            .await
    }
}


struct Health {
    queue_tx: mpsc::Sender<Box<EnclaveQueueMsg>>,
}

#[tonic::async_trait]
impl AppHealthCheckable for Health {
   ...
}
