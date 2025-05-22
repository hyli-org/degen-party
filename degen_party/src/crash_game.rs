use anyhow::{bail, Result};
use board_game::{
    game::{MinigameResult, PlayerMinigameResult},
    GameActionBlob,
};
use crash_game::{
    ChainAction, ChainActionBlob, ChainEvent, GameState, MinigameState, ServerAction, ServerEvent,
};
use hyle_modules::bus::{BusClientSender, SharedMessageBus};
use hyle_modules::modules::websocket::{WsBroadcastMessage, WsInMessage};
use hyle_modules::modules::Module;
use hyle_modules::{module_bus_client, module_handle_messages};
use rand;
use sdk::verifiers::Secp256k1Blob;
use sdk::{
    Blob, BlobIndex, BlobTransaction, ContractAction, ContractName, Identity, StructuredBlobData,
};
use secp256k1::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::time;
use tracing::info;
use uuid;

use crate::{
    game_state::GameStateCommand, AuthenticatedMessage, CryptoContext, InboundWebsocketMessage,
    OutboundWebsocketMessage,
};

// Message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CrashGameCommand {
    CashOut { player_id: Identity },
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CrashGameEvent {
    StateUpdated {
        state: Option<GameState>,
        events: Vec<ChainEvent>,
    },
}

// Bus client definition
module_bus_client! {
pub struct CrashGameBusClient {
    sender(CrashGameCommand),
    sender(GameStateCommand),
    sender(WsBroadcastMessage<OutboundWebsocketMessage>),
    sender(BlobTransaction),
    receiver(CrashGameCommand),
    receiver(WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>),
    receiver(BlobTransaction),
}
}

pub struct CrashGameModule {
    bus: CrashGameBusClient,
    crypto: Arc<CryptoContext>,
    board_game: ContractName,
    crash_game: ContractName,
    update_interval: Duration,

    state: Option<GameState>,
}

impl CrashGameModule {
    async fn handle_player_message(
        &mut self,
        event: CrashGameCommand,
        identity: Identity,
        uuid: &str,
        identity_blobs: Vec<Blob>,
    ) -> Result<()> {
        let uuid_128: u128 = uuid::Uuid::parse_str(uuid)?.as_u128();
        let mut blobs = match event {
            CrashGameCommand::CashOut { player_id } => {
                self.handle_cash_out(uuid_128, player_id).await
            }
            CrashGameCommand::End => self.handle_end(uuid_128).await,
        }?;
        // Merge blobs with identity blobs
        blobs.extend(identity_blobs);
        let tx = BlobTransaction::new(identity, blobs);
        self.bus.send(tx)?;
        Ok(())
    }

    // Pre-chain validation and transaction submission
    async fn handle_cash_out(&mut self, uuid_128: u128, player_id: Identity) -> Result<Vec<Blob>> {
        // Pre-chain validation
        let current_multiplier = self
            .state
            .as_ref()
            .map(|state| state.minigame_backend.current_multiplier);

        let Some(multiplier) = current_multiplier else {
            bail!("Game not initialized");
        };
        Ok(vec![ChainActionBlob(
            uuid_128,
            ChainAction::CashOut {
                player_id,
                multiplier,
            },
        )
        .as_blob(self.crash_game.clone(), None, None)])
    }

    async fn handle_end(&mut self, uuid_128: u128) -> Result<Vec<Blob>> {
        // Pre-chain validation
        match &self.state {
            Some(state) => {
                if state.minigame_verifiable.state != MinigameState::Crashed {
                    bail!("Game is still running");
                }
            }
            None => bail!("Game not initialized"),
        }

        // Get end results from server-side state
        let events = self
            .state
            .as_mut()
            .unwrap()
            .process_server_action(ServerAction::GetEndResults)?;
        let Some(ServerEvent::MinigameEnded { final_results }) = events.last() else {
            bail!("No minigame ended event");
        };

        Ok(vec![
            ChainActionBlob(uuid_128, ChainAction::Done).as_blob(
                self.crash_game.clone(),
                None,
                Some(vec![BlobIndex(1)]),
            ),
            GameActionBlob(
                uuid_128,
                board_game::game::GameAction::EndMinigame {
                    result: MinigameResult {
                        contract_name: self.crash_game.clone(),
                        player_results: final_results
                            .iter()
                            .map(|r| PlayerMinigameResult {
                                player_id: r.0.clone(),
                                coins_delta: r.1,
                            })
                            .collect(),
                    },
                },
            )
            .as_blob(self.board_game.clone(), Some(BlobIndex(0)), None),
        ])
    }

    fn create_backend_identity_blob(&self, uuid: uuid::Uuid, data_to_sign: &str) -> Result<Blob> {
        let identity = Identity::new(format!("{}@secp256k1", self.crypto.public_key));
        let data = format!("{}:{}", uuid, data_to_sign).as_bytes().to_vec();
        let mut hasher = Sha256::new();
        hasher.update(data.clone());
        let message_hash: [u8; 32] = hasher.finalize().into();
        let signature = self
            .crypto
            .secp
            .sign_ecdsa(Message::from_digest(message_hash), &self.crypto.secret_key);
        Ok(Secp256k1Blob::new(
            identity,
            &data,
            &self.crypto.public_key.to_string(),
            &signature.to_string(),
        )?
        .as_blob())
    }

    // Server-side state management
    fn create_backend_tx(&self, action: ChainAction) -> Result<BlobTransaction> {
        let uuid = uuid::Uuid::new_v4();
        let identity = Identity::new(format!("{}@secp256k1", self.crypto.public_key));
        let identity_blob = self.create_backend_identity_blob(
            uuid,
            match action {
                ChainAction::Start => "Start",
                ChainAction::Crash { .. } => "Crash",
                _ => unreachable!(),
            },
        )?;
        Ok(BlobTransaction::new(
            identity.clone(),
            vec![
                identity_blob,
                ChainActionBlob(uuid.as_u128(), action).as_blob(
                    self.crash_game.clone(),
                    None,
                    None,
                ),
            ],
        ))
    }

    async fn handle_update(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };

        if state.minigame_verifiable.state == MinigameState::WaitingForStart {
            // After a while start
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            if now.saturating_sub(state.minigame_backend.game_setup_time.unwrap()) > 10_000 {
                self.bus.send(self.create_backend_tx(ChainAction::Start)?)?;
                return Ok(());
            }
        } else if state.minigame_verifiable.state == MinigameState::Crashed {
            // Auto-end the game after a while to unstuck players
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            if now.saturating_sub(state.minigame_backend.game_start_time.unwrap()) > 60_000 {
                let uuid = uuid::Uuid::new_v4();
                let identity = Identity::new(format!("{}@secp256k1", self.crypto.public_key));
                let mut blobs = self.handle_end(uuid.as_u128()).await?;
                blobs.push(self.create_backend_identity_blob(uuid, "EndMinigame")?);
                self.bus.send(BlobTransaction::new(identity, blobs))?;
                return Ok(());
            }
            return Ok(());
        }

        if state.minigame_verifiable.state != MinigameState::Running {
            return Ok(());
        }

        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let delta = now.saturating_sub(state.minigame_backend.current_time.unwrap());
        state.minigame_backend.current_time = Some(now);
        let elapsed_ms = now.saturating_sub(state.minigame_backend.game_start_time.unwrap());

        // We don't actually send server events for now.
        let _events = state.process_server_action(ServerAction::Update {
            current_time: elapsed_ms as u64,
        })?;

        // Crash probability calculation: from 1% to 50% after 8 seconds (and staying there)
        let elapsed_secs = elapsed_ms as f64 / 1000.0;
        let crash_probability = 0.01 + (0.49 * (elapsed_secs / 8.0).min(1.0));

        // Instant probability over delta ms
        let crash_probability = crash_probability * (delta as f64 / 1000.0);

        info!(
            "Updating game state - {}, {}",
            elapsed_ms, crash_probability
        );

        let state = state.clone();

        if rand::random::<f64>() < crash_probability {
            self.bus.send(self.create_backend_tx(ChainAction::Crash {
                final_multiplier: state.minigame_backend.current_multiplier,
            })?)?;
        }

        self.broadcast_state_update(state, vec![])?;
        Ok(())
    }

    // Chain event processing
    async fn handle_tx(&mut self, tx: BlobTransaction) -> Result<()> {
        for blob in &tx.blobs {
            if blob.contract_name != self.crash_game.clone() {
                continue;
            }

            if let Ok(StructuredBlobData::<ChainActionBlob> { parameters, .. }) =
                StructuredBlobData::<ChainActionBlob>::try_from(blob.data.clone())
            {
                tracing::warn!("Received ChainActionBlob: {:?}", parameters);
                if let ChainAction::InitMinigame { .. } = parameters.1 {
                    let mut state = GameState::new(
                        self.board_game.clone(),
                        Identity::new(format!("{}@secp256k1", self.crypto.public_key)),
                    );
                    state.minigame_backend.game_setup_time =
                        Some(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
                    state.minigame_backend.current_time = state.minigame_backend.game_setup_time;
                    self.state = Some(state);
                } else if let ChainAction::Start = parameters.1 {
                    if let Some(state) = &mut self.state {
                        state.minigame_backend.game_start_time =
                            Some(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
                        state.minigame_backend.current_time =
                            state.minigame_backend.game_start_time;
                    }
                }

                // rust makes this so bad
                let mut events = None;
                if let Some(state) = &mut self.state {
                    // TODO: this is actually unsafe if the transactions between both contracts don't match...
                    events = Some(state.process_chain_action(&tx.identity, &parameters.1, None)?);
                }
                if let Some(state) = &self.state {
                    if let Some(events) = events {
                        self.broadcast_state_update(state.clone(), events)?;
                    }
                }
            } else {
                tracing::warn!("Failed to parse blob as ChainActionBlob");
            }
        }
        Ok(())
    }

    // Helper methods
    fn broadcast_state_update(&mut self, state: GameState, events: Vec<ChainEvent>) -> Result<()> {
        self.bus.send(WsBroadcastMessage {
            message: OutboundWebsocketMessage::CrashGame(CrashGameEvent::StateUpdated {
                state: Some(state),
                events,
            }),
        })?;
        Ok(())
    }
}

impl Module for CrashGameModule {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        let bus = CrashGameBusClient::new_from_bus(bus.new_handle()).await;

        Ok(Self {
            bus,
            state: None,
            update_interval: Duration::from_millis(50),
            crypto: ctx.crypto.clone(),
            board_game: ctx.board_game.clone(),
            crash_game: ctx.crash_game.clone(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut update_interval = time::interval(self.update_interval);

        module_handle_messages! {
            on_bus self.bus,
            listen<WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>> msg => {
                let AuthenticatedMessage {
                    message,
                    identity,
                    uuid,
                    identity_blobs,
                } = msg.message;
                if let InboundWebsocketMessage::CrashGame(event) = message {
                    if let Err(e) = self.handle_player_message(event, identity, &uuid, identity_blobs).await {
                        tracing::warn!("Error handling player message: {:?}", e);
                    }
                }
            }
            listen<BlobTransaction> tx => {
                if let Err(e) = self.handle_tx(tx).await {
                    tracing::warn!("Error handling tx: {:?}", e);
                }
            }
            _ = update_interval.tick() => {
                if let Err(e) = self.handle_update().await {
                    tracing::warn!("Error handling update: {:?}", e);
                }
            }
        };

        Ok(())
    }
}
