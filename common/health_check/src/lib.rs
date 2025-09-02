//! K8s compatible health check service. To use the health check service, something must
//! implement [`AppHealthCheckable`].

use std::sync::Arc;
use tonic_health::{
    pb::health_server::HealthServer,
    server::{HealthReporter, HealthService},
    ServingStatus,
};

/// Re-export the tonic health crate proto based types.
pub use tonic_health::pb;

/// k8s terminology to check if a service is up, but not necessarily ready to serve traffic.
pub const LIVENESS: &str = "liveness";
/// k8s terminology to check if a service is ready to serve traffic.
pub const READINESS: &str = "readiness";

// Duration of sleep between app probes in seconds.
const APP_PROBE_SLEEP_S: u64 = 5;

/// Something that can perform a health check on an app over a socket client.
#[tonic::async_trait]
pub trait AppHealthCheckable {
    /// Perform a health check on a enclave app.
    async fn app_health_check(&self) -> Result<tonic::Response<AppHealthResponse>, tonic::Status>;
}

/// Response to app_health_check
#[derive(Clone, PartialEq)]
pub struct AppHealthResponse {
    /// HTTP status code. Assumes the only health response is 200 for backwards compatibility.
    pub code: i32,
}

/// Spawn a backgrounds process to update the k8s `readiness` status and return the `HealthServer`
/// gRPC service. This will probe the `app_check` every `APP_PROBE_SLEEP_S` seconds
/// and update the health service with its response.
pub async fn spawn_k8s_health_checker<T>(app_check: Arc<T>) -> HealthServer<HealthService>
where
    T: AppHealthCheckable + Send + Sync + 'static,
{
    let reporter = HealthReporter::new();
    let service = HealthService::from_health_reporter(reporter.clone());
    let server = HealthServer::new(service);

    reporter
        .set_service_status(LIVENESS, ServingStatus::Serving)
        .await;
    reporter
        .set_service_status(READINESS, ServingStatus::NotServing)
        .await;

    tokio::task::spawn(async move {
        loop {
            let status = match app_check
                .app_health_check()
                .await
                .map(|resp| match resp.into_inner().code {
                    200 => ServingStatus::Serving,
                    _ => ServingStatus::NotServing,
                })
                .map_err(|_status| ServingStatus::NotServing)
            {
                Ok(s) | Err(s) => s,
            };
            reporter.set_service_status(READINESS, status).await;

            tokio::time::sleep(tokio::time::Duration::from_secs(APP_PROBE_SLEEP_S)).await
        }
    });

    server
}
