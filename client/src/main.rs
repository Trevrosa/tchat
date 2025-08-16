use std::io::Write;

use futures_util::{SinkExt, StreamExt};
use reqwest_websocket::{Message, websocket};
use tokio::task;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    {
        let mut stdout = std::io::stdout();
        write!(stdout, "username: ")?;
        stdout.flush()?;
    }

    let mut user = String::new();
    std::io::stdin().read_line(&mut user)?;
    let user = user.trim();

    let ws = websocket("http://0.0.0.0:7123").await?;
    let (mut tx, mut rx) = ws.split();

    tx.send(Message::Text(user.to_string())).await?;

    tracing::info!("connected as {user}!");

    let receiver = tokio::spawn(async move {
        while let Some(msg) = rx.next().await {
            let Ok(msg) = msg else {
                tracing::warn!("got err while receiving: {}", msg.unwrap_err());
                continue;
            };
            let Message::Text(msg) = msg else {
                tracing::warn!("received non-text message");
                continue;
            };

            println!("{msg}");
        }

        tracing::warn!("stopped listening for msgs");
    });

    let sender = task::spawn_blocking(async move || {
        loop {
            let mut msg = String::new();
            let _ = std::io::stdin().read_line(&mut msg);

            let send = tx.send(Message::Text(msg)).await;
            if let Err(err) = send {
                tracing::error!("failed to send: {err}");
                break;
            }
        }
    }).await?;

    let _ = tokio::join!(receiver, sender);

    Ok(())
}
