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
    // This one could be handled by the server, TODO
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

pub struct CrashGameModule {
    bus: CrashGameBusClient,
    state: Option<GameState>,
    update_interval: Duration,
    game_start_time: Option<Instant>,
}

impl CrashGameModule {
    async fn handle_initialize(
        &mut self,
        players: Vec<(Identity, String, Option<u64>)>,
    ) -> Result<()> {
        let tx = BlobTransaction::new(
            "toto.crash_game",
            vec![ChainActionBlob(
                uuid::Uuid::new_v4().to_string(),
                ChainAction::InitMinigame { players },
            )
            .as_blob("crash_game".into(), None, None)],
        );

        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_place_bet(&mut self, player_id: Identity, amount: u64) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };

        // First validate the bet
        if let Err(err) = state.validate_bet(player_id.clone(), amount) {
            tracing::warn!("Invalid bet: {:?}", err);
            return Ok(());
        }

        let tx = BlobTransaction::new(
            "toto.crash_game",
            vec![ChainActionBlob(
                uuid::Uuid::new_v4().to_string(),
                ChainAction::PlaceBet { player_id, amount },
            )
            .as_blob("crash_game".into(), None, None)],
        );

        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_start(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };

        // Start the game
        let events = state.process_server_action(ServerAction::Start)?;
        if !events.is_empty() {
            self.game_start_time = Some(Instant::now());
            self.bus.send(OutboundWebsocketMessage::CrashGame(
                CrashGameEvent::StateUpdated {
                    state: Some(state.clone()),
                    events: vec![],
                },
            ))?;
        }
        Ok(())
    }

    async fn handle_cash_out(&mut self, player_id: Identity) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let Some(minigame) = &state.current_minigame else {
            return Ok(());
        };

        let current_multiplier = minigame.current_multiplier;
        let tx = BlobTransaction::new(
            "toto.crash_game",
            vec![ChainActionBlob(
                uuid::Uuid::new_v4().to_string(),
                ChainAction::CashOut {
                    player_id,
                    multiplier: current_multiplier,
                },
            )
            .as_blob("crash_game".into(), None, None)],
        );

        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_end(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let Some(minigame) = &state.current_minigame else {
            return Ok(());
        };

        // If we haven't crashed, refuse
        if minigame.is_running {
            return Ok(());
        }

        let mut events = state.process_server_action(ServerAction::GetEndResults)?;
        let Some(ServerEvent::MinigameEnded { final_results }) = events.pop() else {
            bail!("No minigame ended event");
        };

        let uuid = uuid::Uuid::new_v4().to_string();
        let tx = BlobTransaction::new(
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
        );

        self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        Ok(())
    }

    async fn handle_update(&mut self) -> Result<()> {
        let Some(state) = &mut self.state else {
            return Ok(());
        };
        let Some(minigame) = &mut state.current_minigame else {
            return Ok(());
        };
        if !minigame.is_running {
            return Ok(());
        };

        let Some(start_time) = self.game_start_time else {
            return Ok(());
        };

        let elapsed = start_time.elapsed();
        let current_time = elapsed.as_millis() as u64;

        let _events = state.process_server_action(ServerAction::Update { current_time })?;

        // Decide if we should crash based on current time
        let elapsed_secs = current_time as f64 / 1000.0;
        let crash_probability = (elapsed_secs * 0.01).min(0.95); // Increases by 10% per second, max 95%

        info!(
            "Updating game state - {}, {}",
            current_time, crash_probability
        );

        // Debug: never crash before a couple secs
        if rand::random::<f64>() < crash_probability && elapsed_secs > 2.0 {
            // Send crash event as transaction
            let tx = BlobTransaction::new(
                "toto.crash_game",
                vec![ChainActionBlob(
                    uuid::Uuid::new_v4().to_string(),
                    ChainAction::Crash {
                        final_multiplier: state
                            .current_minigame
                            .as_ref()
                            .unwrap()
                            .current_multiplier,
                    },
                )
                .as_blob("crash_game".into(), None, None)],
            );
            self.bus.send(InboundTxMessage::NewTransaction(tx))?;
        }

        // Send state update through websocket
        self.bus.send(OutboundWebsocketMessage::CrashGame(
            CrashGameEvent::StateUpdated {
                state: Some(state.clone()),
                events: vec![], // We don't send server events through StateUpdated
            },
        ))?;
        Ok(())
    }

    async fn handle_tx(&mut self, tx: BlobTransaction) -> Result<()> {
        // Transaction confirmed, now we can update the state
        for blob in &tx.blobs {
            if blob.contract_name != ContractName::from("crash_game") {
                continue;
            }
            // parse as structured blob of ChainActionBlob
            let t = StructuredBlobData::<ChainActionBlob>::try_from(blob.data.clone());
            if let Ok(StructuredBlobData::<ChainActionBlob> { parameters, .. }) = t {
                tracing::debug!("Received blob: {:?}", parameters);
                let events = self.apply_chain_action(&parameters.1).await?;
                self.bus.send(OutboundWebsocketMessage::CrashGame(
                    CrashGameEvent::StateUpdated {
                        state: self.state.clone(),
                        events,
                    },
                ))?;
            } else {
                tracing::warn!("Failed to parse blob as ChainActionBlob");
            }
        }
        Ok(())
    }

    async fn apply_chain_action(&mut self, action: &ChainAction) -> Result<Vec<ChainEvent>> {
        if let ChainAction::InitMinigame { .. } = &action {
            // Initialize the game state
            self.state = Some(GameState::new());
        } else if let ChainAction::Done = &action {
            // End the game
            self.state = None;
            // TODO: process this 'for real'
            return Ok(vec![ChainEvent::MinigameEnded {
                final_results: vec![],
            }]);
        }
        // Apply action optimistically to local state
        match &mut self.state {
            Some(state) => {
                let events = state.process_chain_action(action.clone())?;
                tracing::debug!("Applied action {:?}, got events: {:?}", action, events);

                if let ChainAction::PlaceBet { .. } = &action {
                    // If everyone has placed their bets, start the game
                    if state.ready_to_start() {
                        self.handle_start().await?;
                    }
                }
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
    }
}

impl Module for CrashGameModule {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        let bus = CrashGameBusClient::new_from_bus(ctx.bus.new_handle()).await;

        Ok(Self {
            bus,
            state: None,
            update_interval: Duration::from_millis(50), // 20 updates per second
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
