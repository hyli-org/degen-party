use crash_game::{CrashGameCommand, CrashGameEvent};
use game_state::{GameStateCommand, GameStateEvent};
use serde::{Deserialize, Serialize};

pub mod crash_game;
pub mod fake_lane_manager;
pub mod game_state;

pub struct Context {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedMessage<T> {
    pub message: T,
    pub signature: String,
    pub public_key: String,
    pub message_id: String,
    pub signed_data: String,
}

/// Messages received from WebSocket clients that will be processed by the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum InboundWebsocketMessage {
    GameState(GameStateCommand),
    CrashGame(CrashGameCommand),
}

/// Messages sent to WebSocket clients from the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum OutboundWebsocketMessage {
    GameStateEvent(GameStateEvent),
    CrashGame(CrashGameEvent),
}
