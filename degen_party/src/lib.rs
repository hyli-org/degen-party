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

#[cfg(test)]
mod test {
    use board_game::{game::GameAction, GameActionBlob};
    use crash_game::ChainActionBlob;
    use sdk::{BlobData, StructuredBlobData};

    #[test]
    fn test_blob_parser() {
        let blob = "010100000000000000007eb633198c326b8bd54cd4c0e1cc0c92060a00000063726173685f67616d65040000000b000000616c65784077616c6c657404000000416c65780a000000000000000a0000006d61784077616c6c6574030000004d617832000000000000000a0000006365634077616c6c65740700000043c3a963696c650a000000000000000f0000006c616e63656c6f744077616c6c6574080000004c616e63656c6f741900000000000000";
        let blob_bytes = BlobData(hex::decode(blob).unwrap());
        let Ok(reg) = StructuredBlobData::<GameActionBlob>::try_from(blob_bytes) else {
            panic!("Failed to parse blob data")
        };
        println!("Parsed board blob: {:?}", reg);
        let blob="00010100000000000000000000007eb633198c326b8bd54cd4c0e1cc0c9200040000000b000000616c65784077616c6c657404000000416c65780a000000000000000a0000006d61784077616c6c6574030000004d617832000000000000000a0000006365634077616c6c65740700000043c3a963696c650a000000000000000f0000006c616e63656c6f744077616c6c6574080000004c616e63656c6f741900000000000000";
        let blob_bytes = BlobData(hex::decode(blob).unwrap());
        let Ok(reg) = StructuredBlobData::<ChainActionBlob>::try_from(blob_bytes) else {
            panic!("Failed to parse blob data")
        };
        println!("Parsed crash blob: {:?}", reg);
    }
}
