use std::convert::Infallible;

use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct SseMessage {
    pub event: &'static str,
    pub data: Value,
}

pub fn send_sse_or_cancel(
    tx: &mpsc::UnboundedSender<SseMessage>,
    event: &'static str,
    data: Value,
    cancel: &CancellationToken,
) -> bool {
    if tx.send(SseMessage { event, data }).is_err() {
        cancel.cancel();
        return false;
    }
    true
}

pub fn sse_response(rx: mpsc::UnboundedReceiver<SseMessage>) -> Response {
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
