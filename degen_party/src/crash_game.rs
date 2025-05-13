use anyhow::{bail, Result};
use board_game::{
    game::{MinigameResult, PlayerMinigameResult},
    GameActionBlob,
};
use crash_game::{ChainAction, ChainActionBlob, ChainEvent, GameState, ServerAction, ServerEvent};
use hyle_modules::bus::{BusClientSender, SharedMessageBus};
use hyle_modules::modules::websocket::{WsBroadcastMessage, WsInMessage};
use hyle_modules::modules::Module;
use hyle_modules::{module_bus_client, module_handle_messages};
use rand;
use sdk::verifiers::Secp256k1Blob;
use sdk::{
    Blob, BlobIndex, BlobTransaction, ContractAction, ContractName, Identity, StructuredBlobData,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;
use tracing::info;
use uuid;

use crate::{
    game_state::GameStateCommand, AuthenticatedMessage, InboundWebsocketMessage,
    OutboundWebsocketMessage,
};

// Message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CrashGameCommand {
    Initialize {
        players: Vec<(Identity, String, Option<u64>)>,
    },
    PlaceBet {
        player_id: Identity,
        amount: u64,
    },
    CashOut {
        player_id: Identity,
    },
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
    state: Option<GameState>,
    update_interval: Duration,
    game_start_time: Option<Instant>,
}

impl CrashGameModule {
    async fn handle_player_message(
        &mut self,
        event: CrashGameCommand,
        signature: String,
        public_key: String,
        message_id: String,
        signed_data: String,
    ) -> Result<()> {
        let uuid_128: u128 = uuid::Uuid::parse_str(&message_id)?.as_u128();
        let mut blobs = match event {
            CrashGameCommand::Initialize { players } => {
                self.handle_initialize(uuid_128, players).await
            }
            CrashGameCommand::PlaceBet { player_id, amount } => {
                self.handle_place_bet(uuid_128, player_id, amount).await
            }
            CrashGameCommand::CashOut { player_id } => {
                self.handle_cash_out(uuid_128, player_id).await
            }
            CrashGameCommand::End => self.handle_end(uuid_128).await,
        }?;

        let identity = format!("{public_key}@secp256k1");
        blobs.push(
            Secp256k1Blob::new(
                Identity::from(identity.clone()),
                signed_data.as_bytes(),
                &public_key,
                &signature,
            )?
            .as_blob(),
        );
        let tx = BlobTransaction::new(identity, blobs);
        self.bus.send(tx)?;
        Ok(())
    }

    // Pre-chain validation and transaction submission
    async fn handle_initialize(
        &mut self,
        uuid_128: u128,
        players: Vec<(Identity, String, Option<u64>)>,
    ) -> Result<Vec<Blob>> {
        Ok(vec![ChainActionBlob(
            uuid_128,
            ChainAction::InitMinigame { players },
        )
        .as_blob("crash_game".into(), None, None)])
    }

    async fn handle_place_bet(
        &mut self,
        uuid_128: u128,
        player_id: Identity,
        amount: u64,
    ) -> Result<Vec<Blob>> {
        // Pre-chain validationsa
        if let Some(state) = &mut self.state {
            if let Err(err) = state.validate_bet(player_id.clone(), amount) {
                bail!("Invalid bet: {:?}", err);
            }
        } else {
            bail!("Game not initialized");
        }

        Ok(vec![ChainActionBlob(
            uuid_128,
            ChainAction::PlaceBet { player_id, amount },
        )
        .as_blob("crash_game".into(), None, None)])
    }

    async fn handle_cash_out(&mut self, uuid_128: u128, player_id: Identity) -> Result<Vec<Blob>> {
        // Pre-chain validation
        let current_multiplier = self
            .state
            .as_ref()
            .map(|state| state.minigame.current_multiplier);

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
        .as_blob("crash_game".into(), None, None)])
    }

    async fn handle_end(&mut self, uuid_128: u128) -> Result<Vec<Blob>> {
        // Pre-chain validation
        match &self.state {
            Some(state) => {
                if state.minigame.is_running {
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
                "crash_game".into(),
                None,
                Some(vec![BlobIndex(1)]),
            ),
            GameActionBlob(
                uuid_128,
                board_game::game::GameAction::EndMinigame {
                    result: MinigameResult {
                        contract_name: ContractName("crash_game".into()),
                        player_results: final_results
                            .iter()
                            .map(|r| PlayerMinigameResult {
                                player_id: r.0.clone(),
                                coins_delta: r.1,
                                stars_delta: 0,
                            })
                            .collect(),
                    },
                },
            )
            .as_blob("board_game".into(), Some(BlobIndex(0)), None),
        ])
    }

    // Server-side state management
    async fn handle_start(&mut self) -> Result<()> {
        let Some(_) = &mut self.state else {
            bail!("Game not initialized");
        };

        self.bus.send(BlobTransaction::new(
            "backend@crash_game",
            vec![
                ChainActionBlob(uuid::Uuid::new_v4().as_u128(), ChainAction::Start).as_blob(
                    "crash_game".into(),
                    None,
                    None,
                ),
            ],
        ))?;
        Ok(())
    }

    async fn handle_update(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };

        if !state.minigame.is_running {
            return Ok(());
        }

        let elapsed = self.game_start_time.unwrap().elapsed();
        let current_time = elapsed.as_millis() as u64;

        // We don't actually send server events for now.
        let _events = state.process_server_action(ServerAction::Update { current_time })?;

        // Crash probability calculation
        let elapsed_secs = current_time as f64 / 1000.0;
        let crash_probability = (elapsed_secs * 0.01).min(0.95);

        info!(
            "Updating game state - {}, {}",
            current_time, crash_probability
        );

        if rand::random::<f64>() < crash_probability && elapsed_secs > 2.0 {
            // TODO: auth
            let blobs = vec![ChainActionBlob(
                uuid::Uuid::new_v4().as_u128(),
                ChainAction::Crash {
                    final_multiplier: state.minigame.current_multiplier,
                },
            )
            .as_blob("crash_game".into(), None, None)];
            self.bus
                .send(BlobTransaction::new("backend@crash_game", blobs))?;
        }

        let state = state.clone();
        self.broadcast_state_update(state, vec![])?;
        Ok(())
    }

    // Chain event processing
    async fn handle_tx(&mut self, tx: BlobTransaction) -> Result<()> {
        for blob in &tx.blobs {
            if blob.contract_name != ContractName::from("crash_game") {
                continue;
            }

            if let Ok(StructuredBlobData::<ChainActionBlob> { parameters, .. }) =
                StructuredBlobData::<ChainActionBlob>::try_from(blob.data.clone())
            {
                tracing::debug!("Received blob: {:?}", parameters);
                let events = self.apply_chain_action(&parameters.1).await?;
                if let Some(state) = &self.state {
                    self.broadcast_state_update(state.clone(), events)?;
                }
            } else {
                tracing::warn!("Failed to parse blob as ChainActionBlob");
            }
        }
        Ok(())
    }

    async fn apply_chain_action(&mut self, action: &ChainAction) -> Result<Vec<ChainEvent>> {
        match action {
            ChainAction::InitMinigame { .. } => {
                self.state = Some(GameState::new());
            }
            ChainAction::Start => {
                if self.state.is_some() {
                    self.game_start_time = Some(Instant::now());
                }
            }
            ChainAction::Done => {
                self.state = None;
                return Ok(vec![ChainEvent::MinigameEnded {
                    final_results: vec![],
                }]);
            }
            _ => {}
        }

        // Apply action to state and handle side effects
        match &mut self.state {
            Some(state) => {
                let events = state.process_chain_action(action.clone())?;
                tracing::debug!("Applied action {:?}, got events: {:?}", action, events);

                if let ChainAction::PlaceBet { .. } = action {
                    if state.ready_to_start() {
                        self.handle_start().await?;
                    }
                }
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
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

    async fn build(bus: SharedMessageBus, _ctx: Self::Context) -> Result<Self> {
        let bus = CrashGameBusClient::new_from_bus(bus.new_handle()).await;

        Ok(Self {
            bus,
            state: None,
            update_interval: Duration::from_millis(50),
            game_start_time: None,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut update_interval = time::interval(self.update_interval);

        module_handle_messages! {
            on_bus self.bus,
            listen<WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>> msg => {
                let AuthenticatedMessage {
                    message,
                    signature,
                    public_key,
                    message_id,
                    signed_data,
                } = msg.message;
                if let InboundWebsocketMessage::CrashGame(event) = message {
                    self.handle_player_message(event, signature, public_key, message_id, signed_data).await?;
                }
            }
            listen<BlobTransaction> tx => {
                if let Err(e) = self.handle_tx(tx).await {
                    tracing::warn!("Error handling tx: {:?}", e);
                }
            }
            _ = update_interval.tick() => {
                self.handle_update().await?;
            }
        };

        Ok(())
    }
}
