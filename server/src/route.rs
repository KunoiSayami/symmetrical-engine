use std::{sync::Arc, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    response::IntoResponse,
    Extension, Json,
};
use axum_extra::TypedHeader;
use log::{error, info, warn};
use tap::TapFallible;
use tokio::{
    sync::{broadcast, RwLock},
    time::interval,
};

use crate::{config::Config, types::WebBroadcastEvent, types::WebData};

use super::types::RealIP;

pub async fn route(
    config: Config,
    broadcast: broadcast::Sender<WebBroadcastEvent>,
    users: Arc<RwLock<Vec<String>>>,
) -> anyhow::Result<()> {
    let inner_broadcast = Arc::new(broadcast.clone());

    let router = axum::Router::new()
        .route("/ws/", axum::routing::get(handle_upgrade))
        .route(
            "/",
            axum::routing::get(|| async {
                Json(serde_json::json!({"version": env!("CARGO_PKG_VERSION")}))
            }),
        )
        .layer(Extension(inner_broadcast))
        .layer(Extension(users));

    let listener = tokio::net::TcpListener::bind(config.web().bind()).await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let mut recv = broadcast.subscribe();
            while recv.recv().await.is_ok_and(WebBroadcastEvent::is_not_quit) {}
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        })
        .await?;
    Ok(())
}

pub async fn handle_upgrade(
    ws: WebSocketUpgrade,
    TypedHeader(real_ip): TypedHeader<RealIP>,
    Extension(broadcast): Extension<Arc<broadcast::Sender<WebBroadcastEvent>>>,
    Extension(auth_db): Extension<Arc<RwLock<Vec<String>>>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| async move {
        let ip = real_ip.into_inner();
        info!("Accept request from {ip:?}");
        handle_websocket(socket, broadcast.clone(), &ip, auth_db.clone())
            .await
            .tap_err(|e| error!("Handle {ip} websocket error: {e:?}"))
            .ok();
    })
}

pub async fn handle_websocket(
    mut socket: WebSocket,
    broadcast: Arc<broadcast::Sender<WebBroadcastEvent>>,
    ip: &str,
    auth_db: Arc<RwLock<Vec<String>>>,
) -> anyhow::Result<()> {
    let mut interval = interval(Duration::from_secs(30));
    let mut client_uuid: Option<String> = None;
    let mut receiver = broadcast.subscribe();

    interval.reset();

    loop {
        tokio::select! {
            Ok(event) = receiver.recv() => {
                if client_uuid.is_none() {
                    continue;
                }
                match event {
                    WebBroadcastEvent::RequestTerminate(invoke_uuid) => {
                        if client_uuid.as_ref().unwrap().eq(&invoke_uuid) {
                            info!("Skip self send terminate");
                            continue;
                        }
                        socket.send(Message::Text(format!("terminate {invoke_uuid}"))).await?;
                    }
                    WebBroadcastEvent::ServerQuit => {
                        socket.send(Message::Text("close".to_string())).await.ok();
                        break;
                    }
                }
            }
            Some(message) = socket.recv() => {
                if let Ok(message) = message {
                    if let Ok(text) = message.to_text() {
                        if text.eq("close") {
                            break;
                        }

                        if let Ok(data) = WebData::try_from(text) {
                            match data {
                                WebData::Auth { uuid } => {
                                    if auth_db.read().await.contains(&uuid) {
                                        client_uuid = Some(uuid);
                                        interval.reset_after(Duration::from_secs(114514));
                                    } else {
                                        warn!("ID: {uuid} not in user list");
                                    }
                                },
                                WebData::RequestTerminate => {
                                    match client_uuid {
                                        Some(ref uuid) => {
                                            info!("Receive terminate request from {uuid}");
                                            broadcast
                                                .send(WebBroadcastEvent::RequestTerminate(uuid.clone()))
                                                .ok();
                                        },
                                        None => continue,
                                    }

                                },
                            }
                        }
                    } else {
                        warn!("Skip unreadable bytes: {message:?}");
                    }
                } else {
                    return Ok(());
                }
            }
            _ = interval.tick() => {
                if client_uuid.is_none() {
                    socket.send(Message::Text("auth".to_string())).await?;
                }
            }
        }
    }
    socket.close().await.ok();
    info!("Disconnect from: {ip}");
    Ok(())
}
