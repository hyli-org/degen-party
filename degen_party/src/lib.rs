use std::{path::PathBuf, sync::Arc};

use client_sdk::rest_client::NodeApiHttpClient;
use config::{Config, Environment};
use hyle_modules::modules::websocket::WebSocketConfig;
use rollup_execution::crash_game::{CrashGameCommand, CrashGameEvent};
use rollup_execution::game_state::{GameStateCommand, GameStateEvent};
use sdk::{Blob, ContractName, Identity};
use serde::{Deserialize, Serialize};

pub mod debug;
pub mod ensure_registration;
pub mod fake_lane_manager;
pub mod proving;
pub mod rollup_execution;

pub struct CryptoContext {
    pub secp: secp256k1::Secp256k1<secp256k1::All>,
    pub secret_key: secp256k1::SecretKey,
    pub public_key: secp256k1::PublicKey,
}

pub struct Context {
    pub config: Arc<Conf>,
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

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ContractsConf {
    pub board_game: String,
    pub crash_game: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Conf {
    /// The log format to use - "json", "node" or "full" (default)
    pub log_format: String,
    /// Directory name to store node state.
    pub data_directory: PathBuf,

    pub run_prover: bool,
    pub buffer_blocks: u32,
    pub max_txs_per_proof: usize,
    pub tx_working_window_size: usize,

    pub start_block: u64,

    /// The address of the Rest API to connect to
    pub node_api: String,

    pub contracts: ContractsConf,

    /// When running only the indexer, the address of the DA server to connect to
    pub da_read_from: String,
    /// Websocket configuration
    pub websocket: WebSocketConfig,
}

impl Conf {
    pub fn new(config_files: Vec<String>) -> Result<Self, anyhow::Error> {
        let mut s = Config::builder().add_source(config::File::from_str(
            include_str!("conf_defaults.toml"),
            config::FileFormat::Toml,
        ));
        // Priority order: config file, then environment variables
        for config_file in config_files {
            s = s.add_source(config::File::with_name(&config_file).required(false));
        }
        let conf: Self = s
            .add_source(
                Environment::with_prefix("hyle")
                    .separator("__")
                    .prefix_separator("_"),
            )
            .build()?
            .try_deserialize()?;
        Ok(conf)
    }
}
