//! Primitives for building Turnkey secure app gRPC host servers.

use std::fmt::Debug;
use std::sync::Arc;

use borsh::BorshDeserialize;
use prost::Message;
use qos_core::{
    io::{TimeVal, TimeValLike},
    protocol::{msg::ProtocolMsg, ENCLAVE_APP_SOCKET_CLIENT_TIMEOUT_SECS},
};
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::oneshot,
};
use tonic::Status;

/// Buffer size for socket message queue.
pub static ENCLAVE_QUEUE_CAPACITY: usize = 12;

/// Maximum gRPC message size. Set to 25MB (25*1024*1024)
pub static GRPC_MAX_RECV_MSG_SIZE: usize = 26_214_400;

/// Message sent over socket connection.
pub struct EnclaveQueueMsg<Req, Resp>
where
    Resp: Message + Default,
    Req: Message,
{
    /// Channel to send response back.
    pub response_tx: tokio::sync::oneshot::Sender<Result<Resp, Status>>,
    /// The request message.
    pub request: Req,
}

/// Send a message to secure app via socket connection.
///
/// You likely do not want to transform the error since we want to preserve the
/// unavailable error code to indicate the enclave queue is full.
pub async fn send_queue_msg<Req, Resp>(
    request: Req,
    queue_tx: &tokio::sync::mpsc::Sender<Box<EnclaveQueueMsg<Req, Resp>>>,
) -> Result<Resp, tonic::Status>
where
    Resp: Message + Default,
    Req: Message,
{
    let (response_tx, response_rx) = tokio::sync::oneshot::channel();

    // Send the message to the enclave queue, blocking if the queue is full
    // https://linear.app/turnkey/issue/INF-50/return-unavailable-status-code-if-enclave-queue-is-full
    queue_tx
        .send(Box::new(EnclaveQueueMsg {
            request,
            response_tx,
        }))
        .await
        .map_err(|e| Status::unavailable(format!("send_queue_msg: channel may be full: {e:?}")))?;

    response_rx.await.map_err(|e| {
        Status::internal(format!(
            "send_queue_msg: failed waiting for response: {e:?}"
        ))
    })?
}

/// Send a message to a secure app via QOS proxy.
pub async fn send_proxy_request<Req, Resp>(
    request: Req,
    client: Arc<qos_core::client::Client>,
) -> Result<Resp, tonic::Status>
where
    Resp: Message + Default + Debug + 'static,
    Req: Message,
{
    let qos_request = ProtocolMsg::ProxyRequest {
        data: request.encode_to_vec(),
    };

    // We use spawn_blocking here because `qos_core::client::Client::send` is blocking
    let response = tokio::task::spawn_blocking(move || {
        let encoded_qos_request = borsh::to_vec(&qos_request)
            .map_err(|e| Status::internal(format!("Failed to serialize qos request: {e:?}")))?;

        let encoded_qos_response = client
            .send(&encoded_qos_request)
            .map_err(|e| Status::internal(format!("Failed to query enclave: {e:?}")))?;
        let qos_response = ProtocolMsg::try_from_slice(&encoded_qos_response).map_err(|e| {
            Status::internal(format!("Failed to deserialized enclave response: {e:?}"))
        })?;

        let encoded_app_response = match qos_response {
            ProtocolMsg::ProxyResponse { data } => data,
            other => {
                return Err(Status::internal(format!(
                    "Expected a ProtocolMsg::ProxyResponse but got {other:?}"
                )))
            }
        };

        let response = Resp::decode(&*encoded_app_response)
            .map_err(|e| Status::internal(format!("Failed to decode app response: {e:?}")))?;

        Ok(response)
    })
    .await
    .map_err(|e| Status::internal(format!("Failed to join blocking task: {e:?}")))?;

    response
}

/// Spawn a consumer task to read from the enclave message queue and send messages to the enclave.
pub fn spawn_queue_consumer<Req, Resp>(
    enclave_addr: qos_core::io::SocketAddress,
    mut queue_rx: tokio::sync::mpsc::Receiver<Box<EnclaveQueueMsg<Req, Resp>>>,
) where
    Resp: Message + Default + Debug + 'static,
    Req: Message + 'static,
{
    tokio::task::spawn(async move {
        let client = Arc::new(qos_core::client::Client::new(
            enclave_addr,
            enclave_client_timeout(),
        ));

        loop {
            let queue_msg = queue_rx.recv().await.expect("failed to receive message");
            let enclave_resp = send_proxy_request(queue_msg.request, Arc::clone(&client)).await;

            queue_msg
                .response_tx
                .send(enclave_resp)
                .expect("message processor failed");
        }
    });
}

/// A default timeout for hosts to configure their qos protocol socket client with.
pub fn enclave_client_timeout() -> TimeVal {
    TimeVal::seconds(ENCLAVE_APP_SOCKET_CLIENT_TIMEOUT_SECS * 2)
}

/// Wait for a SIGTERM signal and notify via `sender`.
pub async fn wait_for_sigterm(sender: oneshot::Sender<()>) {
    let _ = signal(SignalKind::terminate())
        .expect("failed to create SIGTERM signal handler")
        .recv()
        .await;
    println!("SIGTERM signal handled, forwarding to host server");
    let _ = sender.send(());
}
