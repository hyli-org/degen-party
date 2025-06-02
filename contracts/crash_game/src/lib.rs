use anyhow::{anyhow, Result};
use board_game::game::{MinigameResult, PlayerMinigameResult};
use board_game::GameActionBlob;
use borsh::{BorshDeserialize, BorshSerialize};
use sdk::caller::ExecutionContext;
use sdk::utils::parse_calldata;
use sdk::{
    Blob, BlobData, BlobIndex, Calldata, ContractAction, ContractName, Identity, LaneId, RunResult,
    StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod utils;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Player {
    pub id: Identity,
    pub name: String,
    pub bet: u64,
    pub cashed_out_at: Option<f64>,
}

#[derive(
    Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq,
)]
pub enum MinigameState {
    #[default]
    Uninitialized,
    WaitingForStart,
    Running,
    Crashed,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MinigameInstanceVerifiable {
    pub state: MinigameState,
    pub players: BTreeMap<Identity, Player>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MinigameInstanceBackend {
    pub current_multiplier: f64,
    pub game_setup_time: Option<u128>,
    pub game_start_time: Option<u128>,
    pub current_time: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GameState {
    pub minigame_verifiable: MinigameInstanceVerifiable,
    pub minigame_backend: MinigameInstanceBackend,
    pub board_contract: ContractName,
    pub backend_identity: Identity,
    pub last_interaction_time: u128,
    pub lane_id: LaneId,
}

// Actions that can be performed on-chain
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum ChainAction {
    InitMinigame {
        players: Vec<(Identity, String, u64)>,
        time: u64,
    },
    Start {
        time: u64,
    },
    CashOut {
        player_id: Identity,
        multiplier: f64,
    },
    Crash {
        final_multiplier: f64,
    },
    Done,
}

// Events that are recorded on-chain
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum ChainEvent {
    MinigameInitialized {
        player_count: usize,
    },
    GameStarted,
    PlayerCashedOut {
        player_id: Identity,
        multiplier: f64,
        winnings: u64,
    },
    GameCrashed {
        final_multiplier: f64,
    },
    MinigameEnded {
        final_results: Vec<(Identity, i32)>,
    },
}

// Server-side actions for real-time updates
#[derive(Debug, Clone)]
pub enum ServerAction {
    Update { current_time: u64 },
}

// Server-side events for UI updates
#[derive(Debug, Clone)]
pub enum ServerEvent {
    MultiplierUpdated {
        multiplier: f64,
    },
    InsufficientFunds {
        player_id: Identity,
        available: u64,
        requested: u64,
    },
    InvalidBetAmount {
        min: u64,
        max: u64,
        provided: u64,
    },
    // Exists just to answer GetEndResults, bit of a hack.
    MinigameEnded {
        final_results: Vec<(Identity, i32)>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChainActionBlob(pub u128, pub ChainAction);

impl ContractAction for ChainActionBlob {
    fn as_blob(
        &self,
        contract_name: ContractName,
        caller: Option<BlobIndex>,
        callees: Option<Vec<BlobIndex>>,
    ) -> Blob {
        Blob {
            contract_name,
            data: BlobData::from(StructuredBlobData {
                caller,
                callees,
                parameters: self.clone(),
            }),
        }
    }
}

impl ZkContract for GameState {
    fn execute(&mut self, contract_input: &Calldata) -> RunResult {
        let (action, mut exec_ctx) =
            parse_calldata::<ChainActionBlob>(contract_input).map_err(|e| e.to_string())?;

        // Not an identity provider
        if contract_input
            .identity
            .0
            .ends_with(&exec_ctx.contract_name.0)
        {
            return Err("Invalid identity provider".to_string());
        }

        let Some(ref ctx) = contract_input.tx_ctx else {
            return Err("Missing transaction context".into());
        };

        // Rollup mode, ensure everything is sent to the same lane ID or we are well past interaction timeout
        let interaction_timeout = ctx.timestamp.0.saturating_add(60 * 60 * 24 * 1000); // 24 hours
        if self.lane_id == LaneId::default() || ctx.timestamp.0 > interaction_timeout {
            self.lane_id = ctx.lane_id.clone();
        } else if self.lane_id != ctx.lane_id {
            return Err("Invalid lane ID".into());
        }

        let events = self
            .process_chain_action(
                &contract_input.identity,
                &action.1,
                Some((&action, &mut exec_ctx)),
            )
            .map_err(|e| e.to_string())?;

        self.last_interaction_time = ctx.timestamp.0;

        Ok((borsh::to_vec(&events).unwrap(), exec_ctx, vec![]))
    }

    fn commit(&self) -> StateCommitment {
        let mut commitment = self.clone();
        commitment.minigame_backend = MinigameInstanceBackend::default();
        StateCommitment(borsh::to_vec(&commitment).unwrap())
    }
}

impl GameState {
    pub fn new(board_contract: ContractName, backend_identity: Identity) -> Self {
        Self {
            minigame_verifiable: MinigameInstanceVerifiable::default(),
            minigame_backend: MinigameInstanceBackend::default(),
            board_contract,
            backend_identity,
            last_interaction_time: 0,
            lane_id: LaneId::default(),
        }
    }

    // Process on-chain actions that need to be recorded
    pub fn process_chain_action(
        &mut self,
        identity: &Identity,
        action: &ChainAction,
        ctx: Option<(&ChainActionBlob, &mut ExecutionContext)>,
    ) -> Result<Vec<ChainEvent>> {
        let mut events = Vec::new();

        match action {
            ChainAction::InitMinigame { players, .. } => {
                if self.minigame_verifiable.state != MinigameState::Uninitialized {
                    return Err(anyhow!("Game is already in progress"));
                }

                // TODO could just read the other blob directly
                if let Some((blob, exec_ctx)) = ctx {
                    // Create a new GameActionBlob with the expected data
                    let expected_board_blob = GameActionBlob(
                        blob.0,
                        board_game::game::GameAction::StartMinigame {
                            minigame: exec_ctx.contract_name.clone(),
                            players: players.clone(),
                        },
                    );
                    // Check our data matches the board contract
                    exec_ctx
                        .is_in_callee_blobs(&self.board_contract, expected_board_blob)
                        .map_err(|_| {
                            anyhow!("Missing or incorrect board game StartMinigame action in transaction",)
                        })?;
                }

                let player_count = players.len();

                // Initialize or update player states
                for (id, name, bet) in players {
                    self.minigame_verifiable.players.insert(
                        id.clone(),
                        Player {
                            id: id.clone(),
                            name: name.clone(),
                            bet: *bet,
                            cashed_out_at: None,
                        },
                    );
                }

                self.minigame_verifiable.state = MinigameState::WaitingForStart;
                self.minigame_backend.current_multiplier = 1.0;

                events.push(ChainEvent::MinigameInitialized { player_count });
            }

            ChainAction::Start { .. } => {
                if identity != &self.backend_identity {
                    return Err(anyhow!(
                        "Only the backend can start the game: {} vs {}",
                        identity,
                        self.backend_identity
                    ));
                }

                if self.minigame_verifiable.state != MinigameState::WaitingForStart {
                    return Err(anyhow!("Game is already in progress"));
                }

                self.minigame_verifiable.state = MinigameState::Running;
                self.minigame_backend.current_multiplier = 1.0;

                events.push(ChainEvent::GameStarted);
            }

            ChainAction::CashOut {
                player_id,
                multiplier,
            } => {
                if self.minigame_verifiable.state != MinigameState::Running {
                    return Err(anyhow!("Game is not running"));
                }

                if identity != player_id {
                    return Err(anyhow!("Player ID does not match the action sender"));
                }

                let Some(player) = self.minigame_verifiable.players.get_mut(player_id) else {
                    return Err(anyhow!("Player not found"));
                };

                if player.cashed_out_at.is_some() {
                    return Err(anyhow!("Bet already cashed out"));
                }

                player.cashed_out_at = Some(*multiplier);

                let winnings = Self::calculate_winnings(player.bet, *multiplier);
                events.push(ChainEvent::PlayerCashedOut {
                    player_id: player_id.clone(),
                    multiplier: *multiplier,
                    winnings,
                });
            }

            ChainAction::Crash { final_multiplier } => {
                if identity != &self.backend_identity {
                    return Err(anyhow!(
                        "Only the backend can start the game: {} vs {}",
                        identity,
                        self.backend_identity
                    ));
                }

                if self.minigame_verifiable.state != MinigameState::Running {
                    return Err(anyhow!("Game is not running"));
                }

                self.minigame_verifiable.state = MinigameState::Crashed;
                self.minigame_backend.current_multiplier = *final_multiplier;

                events.push(ChainEvent::GameCrashed {
                    final_multiplier: *final_multiplier,
                });
            }

            ChainAction::Done => {
                if self.minigame_verifiable.state != MinigameState::Crashed {
                    return Err(anyhow!("Cannot end minigame while it is still running"));
                }
                let expected_final_results = self.final_results();
                if let Some((blob, exec_ctx)) = ctx {
                    // Create a new GameActionBlob with the expected data
                    let expected_board_blob = GameActionBlob(
                        blob.0,
                        board_game::game::GameAction::EndMinigame {
                            result: MinigameResult {
                                contract_name: exec_ctx.contract_name.clone(),
                                player_results: expected_final_results
                                    .iter()
                                    .map(|r| PlayerMinigameResult {
                                        player_id: r.0.clone(),
                                        coins_delta: r.1,
                                    })
                                    .collect(),
                            },
                        },
                    );

                    // When ending the minigame, verify that the board game is being updated with the correct data
                    exec_ctx
                        .is_in_callee_blobs(&self.board_contract, expected_board_blob.clone())
                        .map_err(|_| {
                            anyhow!("Missing board game EndMinigame action in transaction, expected: {:?}", expected_board_blob)
                        })?;
                }

                self.minigame_verifiable = MinigameInstanceVerifiable::default();
                events.push(ChainEvent::MinigameEnded {
                    final_results: expected_final_results,
                });
            }
        }

        Ok(events)
    }

    // Process server-side actions for real-time updates
    pub fn process_server_action(&mut self, action: ServerAction) -> Result<Vec<ServerEvent>> {
        let mut events = Vec::new();

        match action {
            ServerAction::Update { current_time } => {
                if self.minigame_verifiable.state != MinigameState::Running {
                    return Ok(events);
                }

                let new_multiplier = Self::calculate_multiplier(current_time);
                self.minigame_backend.current_multiplier = new_multiplier;

                events.push(ServerEvent::MultiplierUpdated {
                    multiplier: new_multiplier,
                });
            }
        }

        Ok(events)
    }

    fn calculate_multiplier(elapsed_millis: u64) -> f64 {
        let elapsed_secs = elapsed_millis as f64 / 1000.0;
        (elapsed_secs * 0.2).exp()
    }

    fn calculate_winnings(bet_amount: u64, multiplier: f64) -> u64 {
        (bet_amount as f64 * multiplier) as u64
    }

    pub fn get_end_results(&self) -> Result<Vec<(Identity, i32)>> {
        if self.minigame_verifiable.state != MinigameState::Crashed {
            return Err(anyhow!("Game is still running"));
        }
        Ok(self.final_results())
    }

    pub fn final_results(&self) -> Vec<(Identity, i32)> {
        self.minigame_verifiable
            .players
            .iter()
            .map(
                |(
                    id,
                    Player {
                        bet, cashed_out_at, ..
                    },
                )| {
                    let bet = *bet as f64;
                    let delta = if let Some(multiplier) = cashed_out_at {
                        // Player cashed out - calculate profit
                        (bet * multiplier - bet) as i32
                    } else {
                        // Player didn't cash out - lost their bet
                        -bet as i32
                    };
                    (id.clone(), delta)
                },
            )
            .collect::<Vec<_>>()
    }
}
