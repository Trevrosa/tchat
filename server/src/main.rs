mod chat;

use std::sync::Arc;

use axum::{Router, routing::any};
use tokio::{net::TcpListener, sync::broadcast};

use chat::chat;
use tracing::Level;

type Channel = broadcast::Sender<String>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let channel = Arc::new(broadcast::channel(6).0);

    let router = Router::new().route("/", any(chat)).with_state(channel);

    let listener = TcpListener::bind("0.0.0.0:7123").await.unwrap();

    tracing::info!("running server at :7123");

    axum::serve(listener, router).await.unwrap();
}
