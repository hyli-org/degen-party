use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Error, Result};
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
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use hyle::{
    bus::{BusClientSender, BusMessage},
    module_bus_client, module_handle_messages,
    utils::modules::Module,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinSet};
use tracing::{debug, error, info};

use crate::{crash_game::CrashGameCommand, game_state::GameStateEvent};
use crate::{crash_game::CrashGameEvent, game_state::GameStateCommand};

/// Messages received from WebSocket clients that will be processed by the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum InboundWebsocketMessage {
    GameState(GameStateCommand),
    CrashGame(CrashGameCommand),
}

impl BusMessage for InboundWebsocketMessage {}

/// Messages sent to WebSocket clients from the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum OutboundWebsocketMessage {
    GameStateEvent(GameStateEvent),
    CrashGame(CrashGameEvent),
}

impl BusMessage for OutboundWebsocketMessage {}

module_bus_client! {
#[derive(Debug)]
pub struct WebSocketBusClient {
    sender(InboundWebsocketMessage),
    receiver(OutboundWebsocketMessage),
}
}

/// Configuration for the WebSocket module
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// The port number to bind the WebSocket server to
    pub port: u16,
    /// The endpoint path for WebSocket connections
    pub ws_path: String,
    /// The endpoint path for health checks
    pub health_path: String,
    /// The interval at which to check for new peers
    pub peer_check_interval: Duration,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            ws_path: "/ws".to_string(),
            health_path: "/ws_health".to_string(),
            peer_check_interval: Duration::from_millis(100),
        }
    }
}

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

/// A WebSocket module that handles real-time communication for the board game.
/// This module sets up WebSocket endpoints for both the board game and crash game,
/// handling message passing between clients and the game state.
pub struct WebSocketModule {
    bus: WebSocketBusClient,
    app: Option<Router>,
    peer_senders: Vec<SplitSink<WebSocket, Message>>,
    #[allow(clippy::type_complexity)]
    peer_receivers: JoinSet<
        Option<(
            SplitStream<WebSocket>,
            Result<InboundWebsocketMessage, Error>,
        )>,
    >,
    new_peers: NewPeers,
    config: WebSocketConfig,
}

#[derive(Clone, Default)]
struct NewPeers(pub Arc<Mutex<Vec<WebSocket>>>);

impl Module for WebSocketModule {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        let config = WebSocketConfig::default();
        let new_peers = NewPeers::default();
        let app = Router::new()
            .route(&config.ws_path, get(ws_handler))
            .route(&config.health_path, get(health_check))
            .with_state(new_peers.clone());

        Ok(Self {
            bus: WebSocketBusClient::new_from_bus(ctx.bus.new_handle()).await,
            app: Some(app),
            peer_senders: Vec::new(),
            peer_receivers: JoinSet::new(),
            new_peers,
            config,
        })
    }

    async fn run(&mut self) -> Result<()> {
        // Start the server
        let bind_addr = format!("0.0.0.0:{}", self.config.port);
        let listener = tokio::net::TcpListener::bind(&bind_addr)
            .await
            .map_err(|e| {
                WebSocketError::ServerError(format!(
                    "Failed to bind to port {}: {}",
                    self.config.port, e
                ))
            })?;

        info!("WebSocket server listening on port {}", self.config.port);

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
            listen<OutboundWebsocketMessage> msg => {
                if let Err(e) = self.handle_outgoing_message(msg).await {
                    error!("Error sending outbound message: {}", e);
                    break;
                }
            }
            Some(Ok(Some(msg))) = self.peer_receivers.join_next() => {
                match msg {
                    (socket_stream, Ok(msg)) => {
                        debug!("Received message: {:?}", msg);
                        if let Err(e) = self.handle_incoming_message(msg).await {
                            error!("Error handling incoming message: {}", e);
                            break;
                        }
                        // Add it again to the receiver
                        self.peer_receivers.spawn(process_websocket_incoming(socket_stream));
                    }
                    (_, Err(e)) => {
                        error!("Error receiving message: {}", e);
                        break;
                    }
                }
            }
            _ = tokio::time::sleep(self.config.peer_check_interval) => {
                // Check for new peers
                let mut peers = self.new_peers.0.lock().await;
                for peer in peers.drain(..) {
                    let (sender, receiver) = peer.split();
                    self.peer_senders.push(sender);
                    self.peer_receivers.spawn(process_websocket_incoming(receiver));
                }
            }
        };

        server.abort_handle().abort();
        Ok(())
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<NewPeers>) -> impl IntoResponse {
    ws.on_upgrade(async move |socket| {
        debug!("New WebSocket connection established");
        let mut state = state.0.lock().await;
        state.push(socket);
    })
}

async fn process_websocket_incoming(
    mut receiver: SplitStream<WebSocket>,
) -> Option<(
    SplitStream<WebSocket>,
    Result<InboundWebsocketMessage, Error>,
)> {
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!("Received message: {:?}", text);
                return Some((
                    receiver,
                    serde_json::from_str::<InboundWebsocketMessage>(text.as_str())
                        .context("Failed to parse message"),
                ));
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
    None
}

impl WebSocketModule {
    async fn handle_incoming_message(
        &mut self,
        msg: InboundWebsocketMessage,
    ) -> Result<(), WebSocketError> {
        self.bus.send(msg).map_err(|e| {
            WebSocketError::BusSendError(format!("Failed to send inbound message: {}", e))
        })?;
        Ok(())
    }

    async fn handle_outgoing_message(
        &mut self,
        msg: OutboundWebsocketMessage,
    ) -> Result<(), WebSocketError> {
        let mut at_least_one_ok = false;

        let text = serde_json::to_string(&msg)
            .map_err(|e| WebSocketError::InvalidMessageFormat(e.to_string()))?;
        let text: Message = Message::Text(text.clone().into());

        let send_futures: Vec<_> = self
            .peer_senders
            .iter_mut()
            .map(|peer| {
                let text = text.clone();
                async move { peer.send(text).await }
            })
            .collect();

        let results = futures::future::join_all(send_futures).await;

        for idx in (0..self.peer_senders.len()).rev() {
            match &results[idx] {
                Ok(_) => {
                    at_least_one_ok = true;
                }
                Err(e) => {
                    debug!("Failed to send message to WebSocket: {}", e);
                    let _ = self.peer_senders.swap_remove(idx);
                }
            }
        }

        if at_least_one_ok {
            Ok(())
        } else {
            Err(WebSocketError::WebSocketSendError(
                "Failed to send message to all WebSocket peers".to_string(),
            ))
        }
    }
}

// Implement conversions from events to OutboundWebsocketMessage
impl From<GameStateEvent> for OutboundWebsocketMessage {
    fn from(event: GameStateEvent) -> Self {
        OutboundWebsocketMessage::GameStateEvent(event)
    }
}

impl From<CrashGameEvent> for OutboundWebsocketMessage {
    fn from(event: CrashGameEvent) -> Self {
        OutboundWebsocketMessage::CrashGame(event)
    }
}

async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
