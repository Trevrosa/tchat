use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{self, CloseFrame, Utf8Bytes, WebSocket, close_code::UNSUPPORTED},
    },
    response::Response,
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use tokio::{
    sync::broadcast::{self, error::RecvError},
    time,
};

use crate::Channel;

pub async fn chat(ws: WebSocketUpgrade, State(channel): State<Arc<Channel>>) -> Response {
    tracing::debug!("received connection");
    ws.on_upgrade(|ws| handle_socket(ws, channel))
}

fn close_frame(code: u16, reason: &str) -> CloseFrame {
    CloseFrame {
        code,
        reason: reason.into(),
    }
}

const TIMEOUT: u16 = 3008;

async fn handle_socket(socket: WebSocket, state: Arc<Channel>) {
    let (mut tx, mut rx) = socket.split();

    let user = time::timeout(Duration::from_secs(2), rx.next()).await;
    let Ok(Some(Ok(user))) = user else {
        let close = close_frame(TIMEOUT, "no user in 2 secs");

        let _ = tx.send(ws::Message::Close(Some(close))).await;
        return;
    };
    let Ok(user) = user.into_text() else {
        let close = close_frame(UNSUPPORTED, "expected a `text` message");

        let _ = tx.send(ws::Message::Close(Some(close))).await;
        return;
    };

    tracing::info!("user `{user}` connected!");

    tokio::spawn(messager(tx, state.subscribe()));
    tokio::spawn(listener(rx, state.clone(), user));
}

async fn listener(mut rx: SplitStream<WebSocket>, channel: Arc<Channel>, user: Utf8Bytes) {
    while let Some(msg) = rx.next().await {
        let Ok(msg) = msg else {
            tracing::warn!("socket closed");
            return;
        };

        let Ok(text) = msg.into_text() else {
            tracing::warn!("received non-text data");
            continue;
        };

        let send = channel.send(format!("{user}: {text}"));
        if send.is_err() {
            tracing::warn!("failed to broadcast msg to channel");
            return;
        }
    }
}

/// forwards messages from `channel` to the `tx` websocket.
async fn messager(
    mut tx: SplitSink<WebSocket, ws::Message>,
    mut channel: broadcast::Receiver<String>,
) {
    loop {
        match channel.recv().await {
            Ok(msg) => {
                let msg = ws::Message::text(msg);
                let send = tx.send(msg).await;
                if let Err(send) = send {
                    tracing::warn!("couldnt send message: {send}");
                    return;
                }
            }
            Err(RecvError::Lagged(lag)) => {
                tracing::warn!("channel lagged {lag} msgs");
            }
            Err(RecvError::Closed) => {
                tracing::warn!("channel closed");
                break;
            }
        }
    }
}
