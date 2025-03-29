use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use hyle::{
    bus::{BusClientSender, SharedMessageBus},
    handle_messages, module_bus_client, module_handle_messages,
    utils::modules::Module,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info};

use crate::crash_game::CrashGameEvent;
use crate::game_state::GameStateEvent;

#[derive(Error, Debug)]
pub enum WebSocketError {
    #[error("Failed to send message to bus: {0}")]
    BusSendError(String),
    #[error("Failed to send message through websocket: {0}")]
    WebSocketSendError(String),
    #[error("Invalid message format: {0}")]
    InvalidMessageFormat(String),
    #[error("Server error: {0}")]
    ServerError(String),
}

// Helper functions for common operations
async fn send_bus_event(
    bus: &mut WebSocketBusClient,
    event: GameStateEvent,
) -> Result<(), WebSocketError> {
    bus.send(event)
        .map_err(|e| WebSocketError::BusSendError(e.to_string()))
        .map(|_| ())
}

async fn send_crash_game_event(
    bus: &mut WebSocketBusClient,
    event: CrashGameEvent,
) -> Result<(), WebSocketError> {
    bus.send(event)
        .map_err(|e| WebSocketError::BusSendError(e.to_string()))
        .map(|_| ())
}

async fn send_ws_message(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    msg: WebSocketMessage,
) -> Result<(), WebSocketError> {
    let text = serde_json::to_string(&msg)
        .map_err(|e| WebSocketError::InvalidMessageFormat(e.to_string()))?;
    sender
        .send(Message::Text(text.into()))
        .await
        .map_err(|e| WebSocketError::WebSocketSendError(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WebSocketMessage {
    GameState(GameStateEvent),
    CrashGame(CrashGameEvent),
}

module_bus_client! {
#[derive(Debug)]
pub struct WebSocketBusClient {
    sender(GameStateEvent),
    sender(CrashGameEvent),
    receiver(GameStateEvent),
    receiver(CrashGameEvent),
}
}

/// A WebSocket module that handles real-time communication for the board game.
/// This module sets up WebSocket endpoints for both the board game and crash game,
/// handling message passing between clients and the game state.
pub struct WebSocketModule {
    bus: WebSocketBusClient,
    app: Option<Router>,
}

struct BusState(pub SharedMessageBus);
impl Clone for BusState {
    fn clone(&self) -> Self {
        Self(self.0.new_handle())
    }
}

impl Module for WebSocketModule {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        let app = Router::new()
            .route("/ws", get(ws_handler))
            .route("/ws_health", get(health_check))
            .with_state(BusState(ctx.bus.new_handle()));

        Ok(Self {
            bus: WebSocketBusClient::new_from_bus(ctx.bus.new_handle()).await,
            app: Some(app),
        })
    }

    async fn run(&mut self) -> Result<()> {
        // Start the server
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
            //let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .map_err(|e| WebSocketError::ServerError(format!("Failed to bind to port: {}", e)))?;

        info!("WebSocket server listening on http://127.0.0.1:8080");

        let app = self
            .app
            .take()
            .ok_or_else(|| WebSocketError::ServerError("Router was already taken".to_string()))?;

        let server = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Server error: {}", e);
            }
        });

        module_handle_messages! {
            on_bus self.bus,
        };

        server.abort_handle().abort();
        Ok(())
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<BusState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.0))
}

async fn handle_socket(socket: WebSocket, bus: SharedMessageBus) {
    let (mut sender, mut receiver) = socket.split();

    let id = uuid::Uuid::new_v4();
    tracing::info!("WebSocket connection established {}", id);

    let mut poll_bus = WebSocketBusClient::new_from_bus(bus.new_handle()).await;
    let mut sender_bus = WebSocketBusClient::new_from_bus(bus.new_handle()).await;
    // Request initial board game state
    if let Err(e) = send_bus_event(&mut sender_bus, GameStateEvent::SendState).await {
        error!("Failed to send initial state request: {}", e);
        return;
    }

    // Handle incoming messages from the WebSocket client
    let _recv_task = {
        let mut sender_bus = WebSocketBusClient::new_from_bus(bus.new_handle()).await;
        tokio::spawn(async move {
            while let Some(msg) = receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received message: {:?}", text);
                        if let Err(e) = handle_incoming_message(&text, &mut sender_bus).await {
                            error!("Error handling incoming message: {}", e);
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => {
                        debug!("Client initiated close");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {} // Ignore other message types
                }
            }
            debug!("Receiver task {} shutting down", id);
        })
    };

    let _send_task = {
        tokio::spawn(async move {
            handle_messages! {
                on_bus poll_bus,
                listen<GameStateEvent> event => {
                    if let Err(e) = handle_outgoing_message(&mut sender, event).await {
                        error!("Error sending game state message: {}", e);
                        break;
                    }
                }
                listen<CrashGameEvent> event => {
                    if let Err(e) = handle_outgoing_message(&mut sender, event).await {
                        error!("Error sending crash game message: {}", e);
                        break;
                    }
                }
            }
            debug!("Sender task {} shutting down", id);
            let _ = sender.send(Message::Close(None)).await;
        })
    };
}

async fn handle_incoming_message(
    text: &str,
    sender_bus: &mut WebSocketBusClient,
) -> Result<(), WebSocketError> {
    let ws_msg = serde_json::from_str::<WebSocketMessage>(text).map_err(|e| {
        WebSocketError::InvalidMessageFormat(format!("Failed to parse message: {}", e))
    })?;

    match ws_msg {
        WebSocketMessage::GameState(event) => {
            debug!("Received game state event: {:?}", event);
            send_bus_event(sender_bus, event).await.map_err(|e| {
                WebSocketError::BusSendError(format!("Failed to send game state event: {}", e))
            })?;
        }
        WebSocketMessage::CrashGame(event) => {
            debug!("Received crash game event: {:?}", event);
            send_crash_game_event(sender_bus, event)
                .await
                .map_err(|e| {
                    WebSocketError::BusSendError(format!("Failed to send crash game event: {}", e))
                })?;
        }
    }
    Ok(())
}

async fn handle_outgoing_message(
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    event: impl Into<WebSocketMessage>,
) -> Result<(), WebSocketError> {
    let ws_msg = event.into();
    send_ws_message(sender, ws_msg).await
}

// Add implementations to convert events to WebSocketMessage
impl From<GameStateEvent> for WebSocketMessage {
    fn from(event: GameStateEvent) -> Self {
        WebSocketMessage::GameState(event)
    }
}

impl From<CrashGameEvent> for WebSocketMessage {
    fn from(event: CrashGameEvent) -> Self {
        WebSocketMessage::CrashGame(event)
    }
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
