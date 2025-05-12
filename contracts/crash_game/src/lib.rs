use anyhow::{anyhow, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use sdk::caller::ExecutionContext;
use sdk::utils::parse_calldata;
use sdk::{
    info, Blob, BlobData, BlobIndex, Calldata, ContractAction, ContractName, Identity, RunResult,
    StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zkprogram::game::{MinigameResult, PlayerMinigameResult};
use zkprogram::GameActionBlob;

pub mod utils;

// Game constants
const BASE_SPEED: f64 = 0.02;
const ACCELERATION: f64 = 0.000015;
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

#[derive(Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MinigameInstance {
    pub is_running: bool,
    pub current_multiplier: f64,
    pub waiting_for_start: bool,
    pub active_bets: HashMap<Identity, ActiveBet>,
    pub players: HashMap<Identity, Player>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GameState {
    pub minigame: MinigameInstance,
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
    Start,
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
    GetEndResults,
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
        info!("Self Pre: {:?}", self);
        info!("Action: {:?}", action);
        let events = if let ChainAction::Done = &action.1 {
            self.process_done(&action, &mut exec_ctx)
        } else {
            self.process_chain_action(action.1)
        }
        .map_err(|e| e.to_string())?;

        info!("Self Post: {:?}, {:?}", self, self.commit());

        let chain_events = events
            .iter()
            .map(|event| event.to_string())
            .collect::<Vec<String>>();
        Ok((chain_events.join("\n"), exec_ctx, vec![]))
    }

    fn commit(&self) -> StateCommitment {
        match self.minigame.is_running {
            true => {
                // While the game is running, don't commit things that are changing quickly.
                let mut serialized_data = borsh::to_vec(&self.minigame.active_bets).unwrap();
                serialized_data.extend(borsh::to_vec(&self.minigame.players).unwrap());
                // Magic data
                serialized_data.extend([0, 1, 2, 3]);
                StateCommitment(serialized_data)
            }
            false => {
                let mut serialized_data = borsh::to_vec(&self).unwrap();
                // Magic data
                serialized_data.extend([3, 2, 1, 0]);
                StateCommitment(serialized_data)
            }
        }
    }
}

impl GameState {
    pub fn new() -> Self {
        Self::default()
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
                if self.minigame.is_running {
                    return Err(anyhow!("A minigame is already in progress"));
                }

                let player_count = players.len();

                // Initialize or update player states
                for (id, name, coins) in players {
                    self.minigame.players.insert(
                        id.clone(),
                        Player {
                            id: id.clone(),
                            name,
                            coins: coins.unwrap_or(STARTING_COINS),
                        },
                    );
                }

                self.minigame.is_running = false;
                self.minigame.waiting_for_start = true;
                self.minigame.current_multiplier = 1.0;

                events.push(ChainEvent::MinigameInitialized { player_count });
            }

            ChainAction::PlaceBet { player_id, amount } => {
                if !self.minigame.waiting_for_start {
                    return Err(anyhow!("Cannot place bet while game is in progress"));
                }

                let player = self
                    .minigame
                    .players
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Player not found"))?;

                if player.coins < amount {
                    return Err(anyhow!("Insufficient funds"));
                }

                player.coins -= amount;
                self.minigame.active_bets.insert(
                    player_id.clone(),
                    ActiveBet {
                        amount,
                        cashed_out_at: None,
                    },
                );

                events.push(ChainEvent::BetPlaced { player_id, amount });
            }

            ChainAction::Start => {
                if !self.minigame.waiting_for_start {
                    return Err(anyhow!("Game is already in progress"));
                }

                if self.minigame.active_bets.is_empty() {
                    return Err(anyhow!("No bets placed"));
                }

                // Check if all players have placed their bets
                if self.minigame.active_bets.len() != self.minigame.players.len() {
                    return Err(anyhow!("Waiting for all players to place their bets"));
                }

                self.minigame.is_running = true;
                self.minigame.waiting_for_start = false;
                self.minigame.current_multiplier = 1.0;

                events.push(ChainEvent::GameStarted);
            }

            ChainAction::CashOut {
                player_id,
                multiplier,
            } => {
                if !self.minigame.is_running {
                    return Err(anyhow!("Game is not running"));
                }

                let bet = self
                    .minigame
                    .active_bets
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Bet not found"))?;

                if bet.cashed_out_at.is_some() {
                    return Err(anyhow!("Bet already cashed out"));
                }

                bet.cashed_out_at = Some(multiplier);

                let winnings = Self::calculate_winnings(bet.amount, multiplier);

                let player = self
                    .minigame
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
                self.minigame.is_running = false;
                self.minigame.current_multiplier = final_multiplier;

                events.push(ChainEvent::GameCrashed { final_multiplier });
            }

            ChainAction::Done => unreachable!("Handled separately"),
        }

        Ok(events)
    }

    pub fn process_done(
        &mut self,
        blob: &ChainActionBlob,
        exec_ctx: &mut ExecutionContext,
    ) -> Result<Vec<ChainEvent>> {
        if self.minigame.is_running {
            return Err(anyhow!("Cannot end minigame while it is still running"));
        }

        let expected_final_results = self.final_results();

        let expected_board_blob = GameActionBlob(
            blob.0,
            zkprogram::game::GameAction::EndMinigame {
                result: MinigameResult {
                    contract_name: ContractName("crash_game".into()),
                    player_results: expected_final_results
                        .iter()
                        .map(|r| PlayerMinigameResult {
                            player_id: r.0.clone(),
                            coins_delta: r.1,
                            stars_delta: 0,
                        })
                        .collect(),
                },
            },
        );

        // When ending the minigame, verify that the board game is also being updated
        exec_ctx
            .is_in_callee_blobs(&ContractName("board_game".into()), expected_board_blob)
            .map_err(|_| anyhow!("Missing board game EndMinigame action in transaction"))?;

        self.minigame = GameState::new().minigame;
        Ok(vec![ChainEvent::MinigameEnded {
            final_results: expected_final_results,
        }])
    }

    // Process server-side actions for real-time updates
    pub fn process_server_action(&mut self, action: ServerAction) -> Result<Vec<ServerEvent>> {
        let mut events = Vec::new();

        match action {
            ServerAction::Update { current_time } => {
                if !self.minigame.is_running {
                    return Ok(events);
                }

                let new_multiplier = Self::calculate_multiplier(current_time);
                self.minigame.current_multiplier = new_multiplier;

                events.push(ServerEvent::MultiplierUpdated {
                    multiplier: new_multiplier,
                });
            }

            ServerAction::GetEndResults => {
                if self.minigame.is_running {
                    return Err(anyhow!("Game is still running"));
                }
                let final_results = self.final_results();
                events.push(ServerEvent::MinigameEnded { final_results });
            }
        }

        Ok(events)
    }

    pub fn ready_to_start(&self) -> bool {
        if self.minigame.waiting_for_start
            && self.minigame.active_bets.len() == self.minigame.players.len()
        {
            return true;
        }
        false
    }

    // Helper function to validate bet amount
    pub fn validate_bet(&self, player_id: Identity, amount: u64) -> Result<(), ServerEvent> {
        if !(MIN_BET..=MAX_BET).contains(&amount) {
            return Err(ServerEvent::InvalidBetAmount {
                min: MIN_BET,
                max: MAX_BET,
                provided: amount,
            });
        }

        if let Some(player) = self.minigame.players.get(&player_id) {
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

    pub fn final_results(&self) -> Vec<(Identity, i32)> {
        self.minigame
            .players
            .iter()
            .map(|(id, player)| (id.clone(), player.coins as i32))
            .collect::<Vec<_>>()
    }
}
