use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt};
use log::warn;
use reqwest_websocket::{CloseCode, Message, RequestBuilderExt, WebSocket};
use tokio::{sync::mpsc, time::Instant};

use crate::{task::kill_process_by_name, TERMINATE_TARGET};

#[derive(Clone, Copy, Debug)]
pub enum WebEvent {
    SendTerminate,
    Stop,
}

pub async fn make_connection(
    remote: String,
    uuid: String,
    receiver: mpsc::Receiver<WebEvent>,
) -> anyhow::Result<()> {
    let response = reqwest::Client::default()
        .get(&remote)
        .upgrade()
        .send()
        .await?;

    let websocket = response.into_websocket().await?;

    handle_websocket(websocket, &uuid, receiver).await?;
    Ok(())
}

pub async fn handle_websocket(
    mut socket: WebSocket,
    uuid: &str,
    mut outer_receiver: mpsc::Receiver<WebEvent>,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    let mut last_seen = Instant::now();
    socket.send(Message::Text(uuid.to_string())).await?;
    let (mut sender, mut receiver) = socket.split();
    loop {
        tokio::select! {
            Some(Ok(msg)) = receiver.next() => {
                match msg {
                    Message::Text(s) => {
                        if s == "auth" {
                            sender.send(Message::Text(uuid.to_string())).await?;
                        }
                    },
                    Message::Pong(_) => {
                        last_seen = Instant::now();
                    },
                    Message::Close { code, reason } => {
                        warn!("Server closed: {code} {reason}");
                        break
                    },
                    _ => {}
                }
            }

            _ = interval.tick() => {
                if  (Instant::now() - last_seen).as_secs() > 30 {
                    log::error!("Server not response in ping check");
                    break;
                }

                sender.send(Message::Ping(vec![])).await?;
            }

            Some(event) = outer_receiver.recv() => {
                match event {
                    WebEvent::Stop => {
                        break
                    }
                    WebEvent::SendTerminate => {
                        std::thread::spawn(|| unsafe { kill_process_by_name(TERMINATE_TARGET) });
                        sender.send(Message::Text("Terminate".to_string())).await?;
                    }
                }
            }
        }
    }

    sender
        .send(Message::Close {
            code: CloseCode::Normal,
            reason: "Normal exit".to_string(),
        })
        .await
        .ok();
    Ok(())
}
