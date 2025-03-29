use crate::crash_game::CrashGameEvent;
use anyhow::Result;
use board_game_engine::game::{GameAction, GameEvent, GameState};
use hyle::{
    bus::{command_response::Query, BusClientSender, BusMessage},
    model::TxHash,
    module_bus_client, module_handle_messages,
    rest::client::NodeApiHttpClient,
    utils::modules::Module,
};
use hyle_contract_sdk::{ContractName, Identity};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum GameStateEvent {
    ActionSubmitted {
        action: GameAction,
    },
    Reset,
    Initialize {
        player_count: u32,
        board_size: u32,
    },
    SendState,
    StateUpdated {
        state: Option<GameState>,
        events: Vec<GameEvent>,
    },
    MinigameEnded {
        contract_name: ContractName,
        final_results: Vec<(Identity, i32)>,
    },
}

impl BusMessage for GameStateEvent {}

#[derive(Clone)]
pub struct QueryGameState;

module_bus_client! {
pub struct GameStateBusClient {
    sender(GameStateEvent),
    sender(CrashGameEvent),
    receiver(Query<QueryGameState, GameState>),
    receiver(GameStateEvent),
}
}

/// The ELF file for the Succinct RISC-V zkVM.
//#[cfg(not(clippy))]
//pub const CONTRACT_ELF: &[u8] = sp1_sdk::include_elf!("board-game-engine");
//#[cfg(clippy)]
pub const CONTRACT_ELF: &[u8] = &[0, 1, 2, 3];

pub struct GameStateModule {
    bus: GameStateBusClient,
    state: Option<GameState>,
    hyle_client: Arc<NodeApiHttpClient>,
    active_minigame: Option<ContractName>,
}

impl Module for GameStateModule {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        let bus = GameStateBusClient::new_from_bus(ctx.bus.new_handle()).await;

        // Initialize Hylé client
        let hyle_client = Arc::new(NodeApiHttpClient::new("http://localhost:4321".to_string())?);

        Ok(Self {
            bus,
            state: None,
            hyle_client,
            active_minigame: None,
        })
    }

    async fn run(&mut self) -> Result<()> {
        module_handle_messages! {
            on_bus self.bus,
            command_response<QueryGameState, GameState> _ => {
                match &self.state {
                    Some(state) => Ok(state.clone()),
                    None => Err(anyhow::anyhow!("Game not initialized"))
                }
            }
            listen<GameStateEvent> event => {
                if let Err(e) = self.handle_event(event).await {
                    tracing::warn!("Error handling event: {:?}", e);
                }
            }
        };

        Ok(())
    }
}

impl GameStateModule {
    async fn handle_event(&mut self, event: GameStateEvent) -> Result<()> {
        match event {
            GameStateEvent::ActionSubmitted { action } => {
                self.handle_action_submitted(action).await
            }
            GameStateEvent::Reset => self.handle_reset().await,
            GameStateEvent::Initialize {
                player_count,
                board_size,
            } => {
                self.initialize(player_count as usize, board_size as usize)
                    .await
            }
            GameStateEvent::SendState => self.handle_send_state().await,
            GameStateEvent::MinigameEnded {
                contract_name,
                final_results,
            } => {
                // Create proper MinigameResult with computed coin deltas
                let player_results = final_results
                    .into_iter()
                    .map(|(player_id, coins_at_end)| {
                        board_game_engine::game::PlayerMinigameResult {
                            player_id: player_id.clone(),
                            coins_delta: coins_at_end
                                - self
                                    .state
                                    .as_ref()
                                    .unwrap()
                                    .players
                                    .iter()
                                    .find(|p| &p.id == &player_id)
                                    .unwrap()
                                    .coins,
                            stars_delta: 0, // Crash game doesn't affect stars
                        }
                    })
                    .collect();

                let result = board_game_engine::game::MinigameResult {
                    contract_name,
                    player_results,
                };

                self.handle_action_submitted(GameAction::EndMinigame { result })
                    .await
            }
            GameStateEvent::StateUpdated { .. } => {
                // Ignore these events
                Ok(())
            }
        }
    }

    async fn handle_action_submitted(&mut self, action: GameAction) -> Result<()> {
        if self.state.is_some() {
            match &action {
                GameAction::StartMinigame { minigame_type } => {
                    if self.active_minigame.is_some() {
                        return Err(anyhow::anyhow!("Minigame already active"));
                    }
                    // Track the minigame start
                    self.active_minigame = Some(minigame_type.clone().into());

                    // If it's the crash game, initialize it
                    if minigame_type == "crash_game" {
                        if let Some(state) = &self.state {
                            let players: Vec<(Identity, String, Option<u64>)> = state
                                .players
                                .iter()
                                .map(|p| (p.id.clone(), p.name.clone(), Some(p.coins as u64)))
                                .collect();
                            self.bus.send(CrashGameEvent::Initialize { players })?;
                        }
                    }
                }
                GameAction::EndMinigame { result } => {
                    // Verify this is the active minigame
                    if let Some(active) = &self.active_minigame {
                        if active != &result.contract_name {
                            return Err(anyhow::anyhow!(
                                "Invalid minigame end: expected {}, got {}",
                                active,
                                result.contract_name
                            ));
                        }
                        self.active_minigame = None;
                    } else {
                        return Err(anyhow::anyhow!("No active minigame to end"));
                    }
                }
                _ => {}
            }

            let _ = self.submit_to_blockchain(&action).await;
            let events = self.apply_action(&action).await?;

            // Notify state update
            self.bus.send(GameStateEvent::StateUpdated {
                state: Some(self.state.as_ref().unwrap().clone()),
                events,
            })?;
        }
        Ok(())
    }

    async fn handle_reset(&mut self) -> Result<()> {
        self.state = None;
        self.active_minigame = None;
        self.bus.send(GameStateEvent::StateUpdated {
            state: None,
            events: vec![],
        })?;
        Ok(())
    }

    async fn handle_send_state(&mut self) -> Result<()> {
        if let Some(state) = &self.state {
            self.bus.send(GameStateEvent::StateUpdated {
                state: Some(state.clone()),
                events: vec![],
            })?;
        }
        Ok(())
    }

    async fn initialize(&mut self, player_count: usize, board_size: usize) -> Result<()> {
        tracing::warn!(
            "Initializing game with {} players and board size {}",
            player_count,
            board_size
        );
        let new_state = GameState::new(player_count, board_size);
        self.state = Some(new_state.clone());
        self.bus.send(GameStateEvent::StateUpdated {
            state: Some(new_state),
            events: vec![],
        })?;

        /*
        // TODO: do this for real but it's slow when changing the ELF regularly.
        // Load the VK from local file
        let vk_path = "vk.bin";
        let vk = if std::path::Path::new(vk_path).exists() {
            std::fs::read(vk_path)?
        } else {
            let client = ProverClient::from_env();
            let (_, vk) = client.setup(CONTRACT_ELF);
            let vk = serde_json::to_vec(&vk).unwrap();
            // Save it locally along with hash of elf
            // Compute the hash of the ELF file
            let mut hasher = Sha256::new();
            hasher.update(CONTRACT_ELF);
            let elf_hash = hasher.finalize();

            // Save the vk and hash locally
            let mut file = File::create("vk_and_hash.bin")?;
            file.write_all(&vk)?;
            file.write_all(&elf_hash)?;
            vk
        };

        // Send the transaction to register the contract
        let register_tx = APIRegisterContract {
            verifier: "sp1-4".into(),
            program_id: hyle_contract_sdk::ProgramId(vk),
            state_commitment: hyle_contract_sdk::HyleContract::commit(&self.state),
            contract_name: "board_game".into(),
        };
        let res = self
            .hyle_client
            .register_contract(&register_tx)
            .await
            .unwrap();

        tracing::warn!("✅ Register contract tx sent. Tx hash: {}", res);
        */
        Ok(())
    }

    async fn apply_action(&mut self, action: &GameAction) -> Result<Vec<GameEvent>> {
        // Apply action optimistically to local state
        match &mut self.state {
            Some(state) => {
                let events = state.process_action(action.clone())?;
                tracing::debug!("Applied action {:?}, got events: {:?}", action, events);
                Ok(events)
            }
            None => Err(anyhow::anyhow!("Game not initialized")),
        }
    }

    async fn submit_to_blockchain(&self, action: &GameAction) -> Result<TxHash> {
        match action {
            GameAction::StartMinigame { minigame_type } => {
                tracing::info!("Starting minigame: {}", minigame_type);
                // TODO: Implement blockchain submission
                Ok("".into())
            }
            GameAction::EndMinigame { result } => {
                tracing::info!(
                    "Ending minigame: {} with {} player results",
                    result.contract_name.0,
                    result.player_results.len()
                );
                // TODO: Implement blockchain submission
                Ok("".into())
            }
            _ => Ok("".into()),
        }
    }
}
