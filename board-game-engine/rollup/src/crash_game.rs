use anyhow::Result;
use crash_game::{ChainAction, ChainEvent, GameState, ServerAction};
use hyle::{
    bus::{BusClientSender, BusMessage},
    module_bus_client, module_handle_messages,
    utils::modules::Module,
};
use hyle_contract_sdk::Identity;
use rand;
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;
use tracing::info;

use crate::game_state::GameStateEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum CrashGameEvent {
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
    Start,
    End,
    StateUpdated {
        state: Option<GameState>,
        events: Vec<ChainEvent>,
    },
}

impl BusMessage for CrashGameEvent {}

module_bus_client! {
pub struct CrashGameBusClient {
    sender(CrashGameEvent),
    sender(GameStateEvent),
    receiver(CrashGameEvent),
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
        let mut state = GameState::new();
        tracing::warn!("Initializing game with {} players", players.len());
        let events = state.process_chain_action(ChainAction::InitMinigame { players })?;
        self.state = Some(state);
        self.bus.send(CrashGameEvent::StateUpdated {
            state: self.state.clone(),
            events,
        })?;
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

        // Process the bet on-chain
        let events = state.process_chain_action(ChainAction::PlaceBet { player_id, amount })?;
        self.bus.send(CrashGameEvent::StateUpdated {
            state: Some(state.clone()),
            events,
        })?;

        // If everyone has placed their bets, start the game
        if state.ready_to_start() {
            self.bus.send(CrashGameEvent::Start)?;
        }

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
            self.bus.send(CrashGameEvent::StateUpdated {
                state: Some(state.clone()),
                events: vec![],
            })?;
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
        let events = state.process_chain_action(ChainAction::CashOut {
            player_id,
            multiplier: current_multiplier,
        })?;
        self.bus.send(CrashGameEvent::StateUpdated {
            state: Some(state.clone()),
            events,
        })?;
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

        // End the minigame
        let end_events = state.process_chain_action(ChainAction::Done)?;
        self.state = None;
        self.game_start_time = None;
        self.bus.send(CrashGameEvent::StateUpdated {
            state: None,
            events: end_events.clone(),
        })?;

        // Extract final results and send them to game state
        let final_results = end_events
            .iter()
            .find_map(|event| {
                if let ChainEvent::MinigameEnded { final_results } = event {
                    Some(final_results.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        self.bus.send(GameStateEvent::MinigameEnded {
            contract_name: "crash_game".into(),
            final_results,
        })?;
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

        if rand::random::<f64>() < crash_probability {
            // Send crash event
            state.process_chain_action(ChainAction::Crash {
                final_multiplier: state.current_minigame.as_ref().unwrap().current_multiplier,
            })?;
        }
        self.bus.send(CrashGameEvent::StateUpdated {
            state: Some(state.clone()),
            events: vec![], // We don't send server events through StateUpdated
        })?;
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
            update_interval: Duration::from_millis(50), // 20 updates per second
            game_start_time: None,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut update_interval = time::interval(self.update_interval);

        module_handle_messages! {
            on_bus self.bus,
            listen<CrashGameEvent> event => {
                if let Err(e) = async {
                    match event {
                        CrashGameEvent::Initialize { players } => self.handle_initialize(players).await,
                        CrashGameEvent::PlaceBet { player_id, amount } => self.handle_place_bet(player_id, amount).await,
                        CrashGameEvent::Start => self.handle_start().await,
                        CrashGameEvent::CashOut { player_id } => self.handle_cash_out(player_id).await,
                        CrashGameEvent::End => self.handle_end().await,
                        CrashGameEvent::StateUpdated { .. } => { Ok(()) }
                    }}.await {
                    tracing::warn!("Error handling event: {:?}", e);
                }
            }
            _ = update_interval.tick() => {
                self.handle_update().await?;
            }
        };

        Ok(())
    }
}
