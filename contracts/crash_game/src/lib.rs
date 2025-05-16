use anyhow::{anyhow, Result};
use board_game::game::{MinigameResult, PlayerMinigameResult};
use board_game::GameActionBlob;
use borsh::{BorshDeserialize, BorshSerialize};
use sdk::caller::ExecutionContext;
use sdk::utils::parse_calldata;
use sdk::{
    secp256k1, Blob, BlobData, BlobIndex, Calldata, ContractAction, ContractName, Identity, LaneId,
    RunResult, StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(
    Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq,
)]
pub enum MinigameState {
    #[default]
    PlacingBets,
    Running,
    Crashed,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct MinigameInstanceVerifiable {
    pub state: MinigameState,
    pub active_bets: HashMap<Identity, ActiveBet>,
    pub players: HashMap<Identity, Player>,
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

        let expected_data = uuid::Uuid::from_u128(action.0).to_string();
        let expected_action_data = match &action.1 {
            ChainAction::InitMinigame { .. } => "StartMinigame",
            ChainAction::PlaceBet { .. } => "PlaceBet",
            ChainAction::Start => "Start",
            ChainAction::CashOut { .. } => "CashOut",
            ChainAction::Crash { .. } => "Crash",
            ChainAction::Done => "EndMinigame",
        };
        secp256k1::CheckSecp256k1::new(
            contract_input,
            format!("{}:{}", expected_data, expected_action_data).as_bytes(),
        )
        .expect()?;

        let events = if let ChainAction::Done = &action.1 {
            self.process_done(&action, &mut exec_ctx)
        } else {
            self.process_chain_action(&contract_input.identity, action.1, ctx.timestamp.0)
        }
        .map_err(|e| e.to_string())?;

        self.last_interaction_time = ctx.timestamp.0;

        let chain_events = events
            .iter()
            .map(|event| event.to_string())
            .collect::<Vec<String>>();
        Ok((chain_events.join("\n"), exec_ctx, vec![]))
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

    fn calculate_multiplier(elapsed_millis: u64) -> f64 {
        let elapsed_secs = elapsed_millis as f64 / 1000.0;
        let speed = BASE_SPEED + (elapsed_millis as f64 * ACCELERATION);
        (elapsed_secs * speed).exp()
    }

    fn calculate_winnings(bet_amount: u64, multiplier: f64) -> u64 {
        (bet_amount as f64 * multiplier) as u64
    }

    // Process on-chain actions that need to be recorded
    pub fn process_chain_action(
        &mut self,
        identity: &Identity,
        action: ChainAction,
        _timestamp: u128, // TODO use
    ) -> Result<Vec<ChainEvent>> {
        let mut events = Vec::new();

        match action {
            ChainAction::InitMinigame { players } => {
                if self.minigame_verifiable.state == MinigameState::Running {
                    return Err(anyhow!("Game is already in progress"));
                }

                let player_count = players.len();

                // Initialize or update player states
                for (id, name, coins) in players {
                    self.minigame_verifiable.players.insert(
                        id.clone(),
                        Player {
                            id: id.clone(),
                            name,
                            coins: coins.unwrap_or(STARTING_COINS),
                        },
                    );
                }

                self.minigame_verifiable.state = MinigameState::PlacingBets;
                self.minigame_backend.current_multiplier = 1.0;

                events.push(ChainEvent::MinigameInitialized { player_count });
            }

            ChainAction::PlaceBet { player_id, amount } => {
                if self.minigame_verifiable.state != MinigameState::PlacingBets {
                    return Err(anyhow!("Cannot place bet while game is in progress"));
                }
                if identity != &player_id {
                    return Err(anyhow!("Player ID does not match the action sender"));
                }

                let player = self
                    .minigame_verifiable
                    .players
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Player not found"))?;

                if player.coins < amount {
                    return Err(anyhow!("Insufficient funds"));
                }

                player.coins -= amount;
                self.minigame_verifiable.active_bets.insert(
                    player_id.clone(),
                    ActiveBet {
                        amount,
                        cashed_out_at: None,
                    },
                );

                events.push(ChainEvent::BetPlaced { player_id, amount });
            }

            ChainAction::Start => {
                if identity != &self.backend_identity {
                    return Err(anyhow!(
                        "Only the backend can start the game: {} vs {}",
                        identity,
                        self.backend_identity
                    ));
                }

                if self.minigame_verifiable.state != MinigameState::PlacingBets {
                    return Err(anyhow!("Game is already in progress"));
                }

                /*
                // TODO: skipped, should be in TxExecutorHandler only
                if self.minigame_verifiable.active_bets.is_empty() {
                    return Err(anyhow!("No bets placed"));
                }

                // Check if all players have placed their bets
                if self.minigame_verifiable.active_bets.len()
                    != self.minigame_verifiable.players.len()
                {
                    return Err(anyhow!("Waiting for all players to place their bets"));
                }
                */

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

                if identity != &player_id {
                    return Err(anyhow!("Player ID does not match the action sender"));
                }

                let bet = self
                    .minigame_verifiable
                    .active_bets
                    .get_mut(&player_id)
                    .ok_or_else(|| anyhow!("Bet not found"))?;

                if bet.cashed_out_at.is_some() {
                    return Err(anyhow!("Bet already cashed out"));
                }

                bet.cashed_out_at = Some(multiplier);

                let winnings = Self::calculate_winnings(bet.amount, multiplier);

                let player = self
                    .minigame_verifiable
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
                self.minigame_backend.current_multiplier = final_multiplier;

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
        if self.minigame_verifiable.state != MinigameState::Crashed {
            return Err(anyhow!("Cannot end minigame while it is still running"));
        }

        let expected_final_results = self.final_results();

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
                            stars_delta: 0,
                        })
                        .collect(),
                },
            },
        );

        // When ending the minigame, verify that the board game is also being updated
        exec_ctx
            .is_in_callee_blobs(&self.board_contract, expected_board_blob)
            .map_err(|_| anyhow!("Missing board game EndMinigame action in transaction"))?;

        self.minigame_verifiable = MinigameInstanceVerifiable::default();
        Ok(vec![ChainEvent::MinigameEnded {
            final_results: expected_final_results,
        }])
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

            ServerAction::GetEndResults => {
                if self.minigame_verifiable.state != MinigameState::Crashed {
                    return Err(anyhow!("Game is still running"));
                }
                let final_results = self.final_results();
                events.push(ServerEvent::MinigameEnded { final_results });
            }
        }

        Ok(events)
    }

    pub fn ready_to_start(&self) -> bool {
        if self.minigame_verifiable.state == MinigameState::PlacingBets
            && self.minigame_verifiable.active_bets.len() == self.minigame_verifiable.players.len()
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

        if let Some(player) = self.minigame_verifiable.players.get(&player_id) {
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
        self.minigame_verifiable
            .active_bets
            .iter()
            .map(|(id, bet)| {
                let delta = if let Some(multiplier) = bet.cashed_out_at {
                    // Player cashed out - calculate profit
                    (bet.amount as f64 * multiplier - bet.amount as f64) as i32
                } else {
                    // Player didn't cash out - lost their bet
                    -(bet.amount as i32)
                };
                (id.clone(), delta)
            })
            .collect::<Vec<_>>()
    }
}
