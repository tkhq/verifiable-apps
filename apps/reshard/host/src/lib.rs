//! gRPC reshard app host service.

use qos_core::io::SocketAddress;

pub mod generated {
    #![allow(missing_docs)]

    #[rustfmt::skip]
    #[path = "services.reshard.v1.rs"]
    pub mod reshard;

    pub const FILE_DESCRIPTOR_SET: &[u8] = std::include_bytes!("generated/descriptor.bin");
}
pub mod cli;
mod host;

/// Configuration for running the reshard gRPC host.
pub struct ReshardHostConfig {
    listen_addr: std::net::SocketAddr,
    enclave_addr: SocketAddress,
}

/// Run the reshard gRPC host
pub async fn run(
    ReshardHostConfig {
        listen_addr,
        enclave_addr,
    }: ReshardHostConfig,
) -> Result<(), tonic::transport::Error> {
    host::listen(listen_addr, enclave_addr).await
}
