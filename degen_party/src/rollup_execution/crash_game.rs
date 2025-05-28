use anyhow::{bail, Result};
use board_game::{
    game::{MinigameResult, PlayerMinigameResult},
    GameActionBlob,
};
use crash_game::{
    ChainAction, ChainActionBlob, ChainEvent, GameState, MinigameState, ServerAction,
};
use hyle_modules::bus::BusClientSender;
use hyle_modules::modules::websocket::WsBroadcastMessage;
use rand;
use sdk::verifiers::Secp256k1Blob;
use sdk::{Blob, BlobIndex, BlobTransaction, ContractAction, Identity};
use secp256k1::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::DerefMut;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;
use uuid;

use crate::{proving::CrashGameExecutor, OutboundWebsocketMessage};

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

impl super::RollupExecutor {
    pub(super) fn get_crash_game(&mut self) -> &mut GameState {
        let bg = self.crash_game.clone();
        &mut self
            .contracts
            .get_mut(&bg)
            .expect("Crash game not initialized")
            .deref_mut()
            .as_any_mut()
            .downcast_mut::<CrashGameExecutor>()
            .expect("Crash game state is not of the expected type")
            .state
    }

    pub(super) async fn handle_player_message(
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
        let multiplier = self.get_crash_game().minigame_backend.current_multiplier;

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
        if self.get_crash_game().minigame_verifiable.state != MinigameState::Crashed {
            bail!("Game is still running");
        }

        // Get end results from server-side state
        let final_results = self.get_crash_game().get_end_results()?;

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
    fn create_crash_backend_tx(&self, action: ChainAction) -> Result<BlobTransaction> {
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

    pub(super) async fn crash_game_on_tick(&mut self) -> Result<()> {
        let state = self.get_crash_game();

        if state.minigame_verifiable.state == MinigameState::WaitingForStart {
            // After a while start
            let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            if now.saturating_sub(state.minigame_backend.game_setup_time.unwrap()) > 10_000 {
                self.bus
                    .send(self.create_crash_backend_tx(ChainAction::Start)?)?;
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
            self.bus
                .send(self.create_crash_backend_tx(ChainAction::Crash {
                    final_multiplier: state.minigame_backend.current_multiplier,
                })?)?;
        }

        self.broadcast_state_update(state, vec![])?;
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
