use anyhow::{bail, Result};
use board_game_engine::{
    game::{MinigameResult, PlayerMinigameResult},
    GameActionBlob,
};
use crash_game::{ChainAction, ChainActionBlob, ChainEvent, GameState, ServerAction, ServerEvent};
use hyle::{
    bus::{BusClientSender, BusMessage},
    module_bus_client, module_handle_messages,
    utils::modules::Module,
};
use hyle_contract_sdk::{BlobIndex, ContractAction};
use hyle_contract_sdk::{BlobTransaction, ContractName, Identity, StructuredBlobData};
use rand;
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;
use tracing::info;
use uuid;

use crate::{
    fake_lane_manager::InboundTxMessage,
    game_state::GameStateCommand,
    websocket::{InboundWebsocketMessage, OutboundWebsocketMessage},
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

impl BusMessage for CrashGameCommand {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CrashGameEvent {
    StateUpdated {
        state: Option<GameState>,
        events: Vec<ChainEvent>,
    },
}

impl BusMessage for CrashGameEvent {}

// Bus client definition
module_bus_client! {
pub struct CrashGameBusClient {
    sender(CrashGameCommand),
    sender(GameStateCommand),
    sender(OutboundWebsocketMessage),
    sender(InboundTxMessage),
    receiver(CrashGameCommand),
    receiver(InboundWebsocketMessage),
    receiver(InboundTxMessage),
}
}

// Helper struct for creating transactions
struct TransactionBuilder;

impl TransactionBuilder {
    fn new_crash_game_tx(action: ChainAction) -> BlobTransaction {
        BlobTransaction::new(
            "toto.crash_game",
            vec![
                ChainActionBlob(uuid::Uuid::new_v4().to_string(), action).as_blob(
                    "crash_game".into(),
                    None,
                    None,
                ),
            ],
        )
    }

    fn new_end_game_tx(final_results: Vec<(Identity, i32)>) -> BlobTransaction {
        let uuid = uuid::Uuid::new_v4().to_string();
        BlobTransaction::new(
            "toto.crash_game",
            vec![
                ChainActionBlob(uuid.clone(), ChainAction::Done).as_blob(
                    "crash_game".into(),
                    None,
                    Some(vec![BlobIndex(1)]),
                ),
                GameActionBlob(
                    uuid,
                    board_game_engine::game::GameAction::EndMinigame {
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
                .as_blob("board_game".into(), None, Some(vec![BlobIndex(0)])),
            ],
        )
    }
}

pub struct CrashGameModule {
    bus: CrashGameBusClient,
    state: Option<GameState>,
    update_interval: Duration,
    game_start_time: Option<Instant>,
}

impl CrashGameModule {
    // Pre-chain validation and transaction submission
    async fn handle_initialize(
        &mut self,
        players: Vec<(Identity, String, Option<u64>)>,
    ) -> Result<()> {
        let tx = TransactionBuilder::new_crash_game_tx(ChainAction::InitMinigame { players });
        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_place_bet(&mut self, player_id: Identity, amount: u64) -> Result<()> {
        // Pre-chain validation
        if let Some(state) = &mut self.state {
            if let Err(err) = state.validate_bet(player_id.clone(), amount) {
                tracing::warn!("Invalid bet: {:?}", err);
                return Ok(());
            }
        } else {
            return Ok(());
        }

        let tx = TransactionBuilder::new_crash_game_tx(ChainAction::PlaceBet { player_id, amount });
        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_cash_out(&mut self, player_id: Identity) -> Result<()> {
        // Pre-chain validation
        let current_multiplier = if let Some(state) = &self.state {
            Some(state.minigame.current_multiplier)
        } else {
            None
        };

        let Some(multiplier) = current_multiplier else {
            return Ok(());
        };

        let tx = TransactionBuilder::new_crash_game_tx(ChainAction::CashOut {
            player_id,
            multiplier,
        });
        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_end(&mut self) -> Result<()> {
        // Pre-chain validation
        match &self.state {
            Some(state) => {
                if state.minigame.is_running {
                    return Ok(());
                }
            }
            None => return Ok(()),
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

        let tx = TransactionBuilder::new_end_game_tx(final_results.clone());
        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    // Server-side state management
    async fn handle_start(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };

        let events = state.process_server_action(ServerAction::Start)?;
        if !events.is_empty() {
            tracing::info!("Crash game started");
            self.game_start_time = Some(Instant::now());
            let state = state.clone();
            self.broadcast_state_update(state, vec![])?;
        }
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
            let tx = TransactionBuilder::new_crash_game_tx(ChainAction::Crash {
                final_multiplier: state.minigame.current_multiplier,
            });
            self.bus.send(InboundTxMessage::NewTransaction(tx))?;
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
        self.bus.send(OutboundWebsocketMessage::CrashGame(
            CrashGameEvent::StateUpdated {
                state: Some(state),
                events,
            },
        ))?;
        Ok(())
    }
}

impl Module for CrashGameModule {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        let bus = CrashGameBusClient::new_from_bus(ctx.bus.new_handle()).await;

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
            listen<InboundWebsocketMessage> msg => {
                if let InboundWebsocketMessage::CrashGame(event) = msg {
                    if let Err(e) = async {
                        match event {
                            CrashGameCommand::Initialize { players } => self.handle_initialize(players).await,
                            CrashGameCommand::PlaceBet { player_id, amount } => self.handle_place_bet(player_id, amount).await,
                            CrashGameCommand::CashOut { player_id } => self.handle_cash_out(player_id).await,
                            CrashGameCommand::End => self.handle_end().await,
                        }}.await {
                        tracing::warn!("Error handling event: {:?}", e);
                    }
                }
            }
            listen<InboundTxMessage> msg => {
                match msg {
                    InboundTxMessage::NewTransaction(tx) => {
                        if let Err(e) = self.handle_tx(tx).await {
                            tracing::warn!("Error handling tx: {:?}", e);
                        }
                    }
                }
            }
            _ = update_interval.tick() => {
                self.handle_update().await?;
            }
        };

        Ok(())
    }
}
