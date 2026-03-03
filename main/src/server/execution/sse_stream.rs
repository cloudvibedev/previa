use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::warn;

use crate::server::models::SseMessage;

pub fn spawn_broadcast_bridge(
    mut subscriber: broadcast::Receiver<SseMessage>,
    tx: mpsc::UnboundedSender<SseMessage>,
    skip_execution_init: bool,
) {
    tokio::spawn(async move {
        loop {
            match subscriber.recv().await {
                Ok(message) => {
                    if skip_execution_init && message.event == "execution:init" {
                        continue;
                    }
                    if tx.send(message).is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(dropped)) => {
                    warn!("SSE subscriber lagged and dropped {dropped} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

pub fn sse_response_from_rx(rx: mpsc::UnboundedReceiver<SseMessage>) -> Response {
    let stream = UnboundedReceiverStream::new(rx).map(|msg| {
        let event = Event::default()
            .event(msg.event)
            .data(serde_json::to_string(&msg.data).unwrap_or_else(|_| "{}".to_owned()));
        Ok::<Event, Infallible>(event)
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
