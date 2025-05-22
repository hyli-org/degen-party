use std::{path::PathBuf, sync::Arc};

use client_sdk::rest_client::NodeApiHttpClient;
use crash_game::{CrashGameCommand, CrashGameEvent};
use game_state::{GameStateCommand, GameStateEvent};
use sdk::{Blob, ContractName, Identity};
use serde::{Deserialize, Serialize};

pub mod crash_game;
pub mod ensure_registration;
pub mod fake_lane_manager;
pub mod game_state;
pub mod proving;

pub struct CryptoContext {
    pub secp: secp256k1::Secp256k1<secp256k1::All>,
    pub secret_key: secp256k1::SecretKey,
    pub public_key: secp256k1::PublicKey,
}

pub struct Context {
    pub client: Arc<NodeApiHttpClient>,
    pub crypto: Arc<CryptoContext>,
    pub data_directory: PathBuf,
    pub board_game: ContractName,
    pub crash_game: ContractName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedMessage<T> {
    pub message: T,
    pub identity: Identity,
    pub uuid: String,
    pub identity_blobs: Vec<Blob>,
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
