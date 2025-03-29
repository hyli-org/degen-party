use anyhow::{anyhow, Context, Result};
use board_game_engine::game::GameAction;
use board_game_engine::GameActionBlob;
use borsh::{BorshDeserialize, BorshSerialize};
use hyle_contract_sdk::utils::parse_contract_input;
use hyle_contract_sdk::{
    info, Blob, BlobData, BlobIndex, ContractAction, ContractInput, ContractName, HyleContract,
    Identity, RunResult, StateCommitment, StructuredBlobData,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod utils;

// Game constants
const BASE_SPEED: f64 = 0.02;
const ACCELERATION: f64 = 0.000015;
const MIN_MULTIPLIER: f64 = 1.2;
const MAX_MULTIPLIER: f64 = 25.0;
const MIN_BET: u64 = 1;
const MAX_BET: u64 = 100;
const STARTING_COINS: u64 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Player {
    pub id: Identity,
    pub name: String,
    pub coins: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ActiveBet {
    pub amount: u64,
    pub cashed_out_at: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MinigameInstance {
    pub is_running: bool,
    pub current_multiplier: f64,
    pub waiting_for_start: bool,
    pub active_bets: HashMap<Identity, ActiveBet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GameState {
    pub players: HashMap<Identity, Player>,
    pub current_minigame: Option<MinigameInstance>,
}

// Actions that can be performed on-chain
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum ChainAction {
    InitMinigame {
        players: Vec<(Identity, String, Option<u64>)>,
    },
    PlaceBet {
        player_id: Identity,
        amount: u64,
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
    BetPlaced {
        player_id: Identity,
        amount: u64,
    },
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
    Start,
    Update { current_time: u64 },
}

// Server-side events for UI updates
#[derive(Debug, Clone)]
pub enum ServerEvent {
    GameStarted,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ChainActionBlob(pub String, pub ChainAction);

// Modify ChainAction to implement ContractAction trait
impl ContractAction for ChainAction {
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

impl HyleContract for GameState {
    fn execute(&mut self, contract_input: &ContractInput) -> RunResult {
        let (action, mut exec_ctx) =
            parse_contract_input::<ChainActionBlob>(contract_input).map_err(|e| e.to_string())?;

        let endgameblob = contract_input.blobs.iter().find(|blob| {
            if blob.contract_name != ContractName("board_game".into()) {
                return false;
            }
            let Some(blob) = StructuredBlobData::<GameActionBlob>::try_from(blob.data.clone()).ok()
            else {
                return false;
            };
            matches!(blob.parameters.1, GameAction::EndMinigame { .. })
        });

        if let ChainAction::Done = &action.1 {
            // When ending the minigame, verify that the board game is also being updated
            exec_ctx
                .is_in_callee_blobs(
                    &ContractName("board_game".into()),
                    endgameblob
                        .ok_or_else(|| {
                            "Missing board game EndMinigame action in transaction".to_string()
                        })?
                        .clone(),
                )
                .map_err(|_| "Missing board game EndMinigame action in transaction".to_string())?;
        }

        let events = self
            .process_chain_action(action.1)
            .map_err(|e| e.to_string())?;

        let chain_events = events
            .iter()
            .map(|event| event.to_string())
            .collect::<Vec<String>>();
        Ok((chain_events.join("\n"), exec_ctx, vec![]))
    }

    fn commit(&self) -> StateCommitment {
        StateCommitment(borsh::to_vec(self).unwrap())
    }
}

impl From<StateCommitment> for GameState {
    fn from(state: StateCommitment) -> Self {
        GameState::try_from_slice(&state.0).unwrap()
    }
}

impl GameState {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            current_minigame: None,
        }
    }

    fn calculate_multiplier(elapsed_millis: u64) -> f64 {
        let elapsed_secs = elapsed_millis as f64 / 1000.0;
        let speed = BASE_SPEED + (elapsed_millis as f64 * ACCELERATION);
        (elapsed_secs * speed).exp()
    }

    fn calculate_winnings(bet_amount: u64, multiplier: f64) -> u64 {
        (bet_amount as f64 * multiplier) as u64
    }

    // Process on-chain actions that need to be recorded
    pub fn process_chain_action(&mut self, action: ChainAction) -> Result<Vec<ChainEvent>> {
        let mut events = Vec::new();

        match action {
            ChainAction::InitMinigame { players } => {
                if self.current_minigame.is_some() {
                    return Err(anyhow!("A minigame is already in progress"));
                }

                let player_count = players.len();

                // Initialize or update player states
                for (id, name, coins) in players {
                    self.players.insert(
                        id.clone(),
                        Player {
                            id: id.clone(),
                            name,
                            coins: coins.unwrap_or(STARTING_COINS),
                        },
                    );
                }

                self.current_minigame = Some(MinigameInstance {
                    is_running: false,
                    current_multiplier: 1.0,
                    waiting_for_start: true,
                    active_bets: HashMap::new(),
                });

                events.push(ChainEvent::MinigameInitialized { player_count });
            }

            ChainAction::PlaceBet { player_id, amount } => {
                let minigame = self
                    .current_minigame
                    .as_mut()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                if !minigame.waiting_for_start {
                    return Err(anyhow!("Cannot place bet while game is in progress"));
                }

                let player = self
                    .players
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Player not found"))?;

                if player.coins < amount {
                    return Err(anyhow!("Insufficient funds"));
                }

                player.coins -= amount;
                minigame.active_bets.insert(
                    player_id.clone(),
                    ActiveBet {
                        amount,
                        cashed_out_at: None,
                    },
                );

                events.push(ChainEvent::BetPlaced { player_id, amount });
            }

            ChainAction::CashOut {
                player_id,
                multiplier,
            } => {
                let minigame = self
                    .current_minigame
                    .as_mut()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                if !minigame.is_running {
                    return Err(anyhow!("Game is not running"));
                }

                let bet = minigame
                    .active_bets
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Bet not found"))?;

                bet.cashed_out_at = Some(multiplier);

                let winnings = Self::calculate_winnings(bet.amount, multiplier);

                let player = self
                    .players
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Player not found"))?;
                player.coins += winnings;

                events.push(ChainEvent::PlayerCashedOut {
                    player_id,
                    multiplier,
                    winnings,
                });
            }

            ChainAction::Crash { final_multiplier } => {
                let minigame = self
                    .current_minigame
                    .as_mut()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                minigame.is_running = false;
                minigame.current_multiplier = final_multiplier;

                events.push(ChainEvent::GameCrashed { final_multiplier });
            }

            ChainAction::Done => {
                let minigame = self
                    .current_minigame
                    .as_ref()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                if minigame.is_running {
                    return Err(anyhow!("Cannot end minigame while it is still running"));
                }

                let final_results: Vec<(Identity, i32)> = self
                    .players
                    .iter()
                    .map(|(id, player)| {
                        let coins_delta = player.coins as i32;
                        (id.clone(), coins_delta)
                    })
                    .collect();

                self.current_minigame = None;
                events.push(ChainEvent::MinigameEnded { final_results });
            }
        }

        Ok(events)
    }

    // Process server-side actions for real-time updates
    pub fn process_server_action(&mut self, action: ServerAction) -> Result<Vec<ServerEvent>> {
        let mut events = Vec::new();

        match action {
            ServerAction::Start => {
                let minigame = self
                    .current_minigame
                    .as_mut()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                if !minigame.waiting_for_start {
                    return Err(anyhow!("Game is already in progress"));
                }

                if minigame.active_bets.is_empty() {
                    return Err(anyhow!("No bets placed"));
                }

                // Check if all players have placed their bets
                if minigame.active_bets.len() != self.players.len() {
                    return Err(anyhow!("Waiting for all players to place their bets"));
                }

                minigame.is_running = true;
                minigame.waiting_for_start = false;
                minigame.current_multiplier = 1.0;

                events.push(ServerEvent::GameStarted);
            }

            ServerAction::Update { current_time } => {
                let minigame = self
                    .current_minigame
                    .as_mut()
                    .ok_or_else(|| anyhow!("No active minigame"))?;

                if !minigame.is_running {
                    return Ok(events);
                }

                let new_multiplier = Self::calculate_multiplier(current_time);
                minigame.current_multiplier = new_multiplier;

                events.push(ServerEvent::MultiplierUpdated {
                    multiplier: new_multiplier,
                });
            }
        }

        Ok(events)
    }

    pub fn ready_to_start(&self) -> bool {
        if let Some(minigame) = &self.current_minigame {
            if minigame.waiting_for_start && minigame.active_bets.len() == self.players.len() {
                return true;
            }
        }
        false
    }

    // Helper function to validate bet amount
    pub fn validate_bet(&self, player_id: Identity, amount: u64) -> Result<(), ServerEvent> {
        if amount < MIN_BET || amount > MAX_BET {
            return Err(ServerEvent::InvalidBetAmount {
                min: MIN_BET,
                max: MAX_BET,
                provided: amount,
            });
        }

        if let Some(player) = self.players.get(&player_id) {
            if player.coins < amount {
                return Err(ServerEvent::InsufficientFunds {
                    player_id,
                    available: player.coins,
                    requested: amount,
                });
            }
        }

        Ok(())
    }
}
