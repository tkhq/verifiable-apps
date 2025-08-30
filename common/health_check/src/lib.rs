//! K8s compliant health check service.
//!
//! A type needs to implement [`AppHealthCheckable`] for [`K8Health`] to auto implement the
//! gRPC [`HealthService`] (the k8s compliant gRPC health check gRPC api).

use crate::generated::k8health::{
    health_check_response::ServingStatus, health_server::Health, health_server::HealthServer,
    HealthCheckRequest, HealthCheckResponse,
};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio_stream::Stream;

pub mod generated {
    #![allow(missing_docs)]

    #[rustfmt::skip]
    #[path = "grpc.health.v1.rs"]
    pub mod k8health;

    pub const FILE_DESCRIPTOR_SET: &[u8] = std::include_bytes!("generated/descriptor.bin");
}

const WATCH_STREAM_TIMEOUT_SEC: u64 = 3;
const STREAM_MSG_BUFFER_MAX: usize = 16;

/// k8s terminology to check if a service is up, but not necessarily ready to serve traffic.
pub const LIVENESS: &str = "liveness";
/// k8s terminology to check if a service is ready to serve traffic.
pub const READINESS: &str = "readiness";

/// Something that can perform a health check on an app over a socket client
#[tonic::async_trait]
pub trait AppHealthCheckable: Clone {
    /// Perform a health check on a enclave app.
    async fn app_health_check(&self) -> Result<tonic::Response<AppHealthResponse>, tonic::Status>;
}

/// GRPC Health Checking Protocol
/// <https://github.com/grpc/grpc/blob/master/doc/health-checking.md>
#[derive(Clone)]
pub struct K8Health<T> {
    app_check: T,
}

/// Response to app_health_check
#[derive(Clone, PartialEq)]
pub struct AppHealthResponse {
    /// HTTP status code. Assumes the only health response is 200 for backwards compatibility.
    pub code: i32,
}

impl<T> K8Health<T>
where
    T: AppHealthCheckable + Send + Sync + 'static,
{
    /// Create a new instance of [`Self`], with the given enclave
    /// (`enclave_addr`).
    #[must_use]
    pub fn build_service(app_check: T) -> HealthServer<K8Health<T>> {
        let inner = Self { app_check };
        HealthServer::new(inner)
    }

    async fn app_status(&self) -> generated::k8health::health_check_response::ServingStatus {
        match self
            .app_check
            .app_health_check()
            .await
            .map(|resp| match resp.into_inner().code {
                200 => ServingStatus::Serving,
                _ => ServingStatus::NotServing,
            })
            .map_err(|_status| ServingStatus::NotServing)
        {
            Ok(s) | Err(s) => s,
        }
    }

    async fn k8_request(
        &self,
        request: &tonic::Request<HealthCheckRequest>,
    ) -> HealthCheckResponse {
        let status = match request.get_ref().service.as_str() {
            LIVENESS => ServingStatus::Serving,
            READINESS => self.app_status().await,
            _ => ServingStatus::ServiceUnknown,
        };

        HealthCheckResponse {
            status: status as i32,
        }
    }
}

type ResponseStream =
    Pin<Box<dyn Stream<Item = Result<HealthCheckResponse, tonic::Status>> + Send>>;

#[tonic::async_trait]
impl<T> Health for K8Health<T>
where
    T: AppHealthCheckable + Send + Sync + 'static,
{
    async fn check(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> std::result::Result<tonic::Response<HealthCheckResponse>, tonic::Status> {
        Ok(tonic::Response::new(self.k8_request(&request).await))
    }

    type WatchStream = ResponseStream;

    async fn watch(
        &self,
        request: tonic::Request<HealthCheckRequest>,
    ) -> std::result::Result<tonic::Response<Self::WatchStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel(STREAM_MSG_BUFFER_MAX);
        let self2 = self.clone();
        tokio::spawn(async move {
            loop {
                let status = self2.k8_request(&request).await;
                if tx.send(Ok(status)).await.is_err() {
                    break;
                }

                tokio::time::sleep(Duration::from_secs(WATCH_STREAM_TIMEOUT_SEC)).await;
            }
        });

        let output_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

        Ok(tonic::Response::new(
            Box::pin(output_stream) as Self::WatchStream
        ))
    }
}
