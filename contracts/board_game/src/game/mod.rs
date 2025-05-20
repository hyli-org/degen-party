use anyhow::{anyhow, bail, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use sdk::{ContractName, Identity, LaneId, StateCommitment};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod dice;
pub mod player;
pub mod utils;

const ROUNDS: usize = 10;
#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GameState {
    pub players: Vec<Player>,
    pub current_turn: usize,
    pub phase: GamePhase,
    pub max_players: usize,
    pub minigames: Vec<ContractName>,
    pub dice: dice::Dice,
    pub round: usize,
    pub bets: HashMap<Identity, u64>,

    // Metadata to ensure the game runs smoothly
    pub backend_identity: Identity,
    pub last_interaction_time: u128,
    pub lane_id: LaneId,
    pub all_or_nothing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Player {
    pub id: Identity,
    pub name: String,
    pub position: usize,
    pub coins: i32,
    pub used_uuids: Vec<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct MinigameResult {
    pub contract_name: ContractName,
    pub player_results: Vec<PlayerMinigameResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct PlayerMinigameResult {
    pub player_id: Identity,
    pub coins_delta: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum GamePhase {
    Registration,
    Betting,
    WheelSpin,
    StartMinigame(ContractName),
    InMinigame(ContractName),
    FinalMinigame(ContractName),
    GameOver,
}

pub type MinigameSetup = Vec<(Identity, String, u64)>;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
pub enum GameAction {
    EndGame,
    Initialize {
        player_count: usize,
        minigames: Vec<String>,
        random_seed: u64,
    },
    RegisterPlayer {
        name: String,
    },
    StartGame,
    PlaceBet {
        amount: u64,
    },
    SpinWheel,
    StartMinigame {
        minigame: ContractName,
        players: MinigameSetup,
    },
    EndMinigame {
        result: MinigameResult,
    },
    EndTurn,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum GameEvent {
    DiceRolled {
        player_id: Identity,
        value: u8,
    },
    PlayerMoved {
        player_id: Identity,
        new_position: usize,
    },
    CoinsChanged {
        player_id: Identity,
        amount: i32,
    },
    MinigameReady {
        minigame_type: String,
    },
    MinigameStarted {
        minigame_type: String,
    },
    MinigameEnded {
        result: MinigameResult,
    },
    TurnEnded {
        next_player: Identity,
    },
    GameEnded {
        winner_id: Identity,
        final_coins: i32,
    },
    GameInitialized {
        player_count: usize,
        random_seed: u64,
    },
    PlayerRegistered {
        name: String,
        player_id: Identity,
    },
    GameStarted,
    BetPlaced {
        player_id: Identity,
        amount: u64,
    },
    WheelSpun {
        outcome: u8,
    },
    PlayersSwappedCoins {
        swaps: Vec<(Identity, Identity)>,
    },
    AllOrNothingActivated,
}

impl From<StateCommitment> for GameState {
    fn from(state: StateCommitment) -> Self {
        GameState::try_from_slice(&state.0).unwrap()
    }
}

impl GameState {
    pub fn new(backend_identity: Identity) -> Self {
        Self {
            players: Vec::new(),
            current_turn: 0,
            phase: GamePhase::GameOver,
            max_players: 4,
            minigames: Vec::new(),
            dice: dice::Dice::new(1, 10, 0),

            backend_identity,
            last_interaction_time: 0,
            lane_id: LaneId::default(),
            round: 0,
            bets: HashMap::new(),
            all_or_nothing: false,
        }
    }

    pub fn reset(&mut self, player_count: usize, minigames: Vec<ContractName>, random_seed: u64) {
        *self = Self {
            players: Vec::with_capacity(player_count),
            current_turn: 0,
            phase: GamePhase::GameOver,
            max_players: player_count,
            minigames,
            dice: dice::Dice::new(1, 10, random_seed),

            backend_identity: self.backend_identity.clone(),
            last_interaction_time: self.last_interaction_time,
            lane_id: self.lane_id.clone(),
            round: self.round,
            bets: self.bets.clone(),
            all_or_nothing: false,
        }
    }

    // Helper function for updating coins and generating events
    fn update_player_coins(
        &mut self,
        player_index: usize,
        delta: i32,
        events: &mut Vec<GameEvent>,
    ) -> Result<()> {
        let Some(player) = self.players.get_mut(player_index) else {
            return Err(anyhow!("Player not found"));
        };
        player.coins = (player.coins + delta).max(0);
        events.push(GameEvent::CoinsChanged {
            player_id: player.id.clone(),
            amount: delta,
        });
        Ok(())
    }

    pub fn get_minigame_setup(&self) -> MinigameSetup {
        self.bets
            .iter()
            .filter_map(|(id, &bet)| {
                self.players
                    .iter()
                    .find(|p| p.id == *id)
                    .map(|p| (p.id.clone(), p.name.clone(), bet))
            })
            .collect()
    }

    // Helper function for handling minigame results
    fn apply_minigame_result(
        &mut self,
        player_index: usize,
        result: &PlayerMinigameResult,
        events: &mut Vec<GameEvent>,
    ) -> Result<()> {
        if result.coins_delta != 0 {
            self.update_player_coins(player_index, result.coins_delta, events)?;
        }
        Ok(())
    }

    fn is_registered(&self, caller: &Identity) -> bool {
        self.players.iter().any(|p| p.id == *caller)
    }

    fn get_current_player(&self) -> Result<&Player> {
        self.players
            .get(self.current_turn % self.players.len())
            .ok_or_else(|| anyhow!("Invalid current turn index"))
    }

    fn advance_turn(&mut self) {
        self.current_turn += 1;
    }

    pub fn process_action(
        &mut self,
        caller: &Identity,
        uuid: u128,
        action: GameAction,
        timestamp: u128,
    ) -> Result<Vec<GameEvent>> {
        let mut events = Vec::new();

        if let Some(player) = self.players.iter_mut().find(|p| &p.id == caller) {
            if player.used_uuids.contains(&uuid) {
                bail!("UUID already used");
            }
            player.used_uuids.push(uuid);
        }

        match (self.phase.clone(), action) {
            (_, GameAction::EndGame) => {
                let is_backend = self.backend_identity == *caller;
                let game_timed_out = timestamp - self.last_interaction_time > 10 * 60 * 1000;
                if is_backend || game_timed_out {
                    events.push(GameEvent::GameEnded {
                        winner_id: Identity::default(),
                        final_coins: 0,
                    });
                    self.reset(4, self.minigames.clone(), self.dice.seed);
                } else {
                    return Err(anyhow!("Only the backend can end the game"));
                }
            }
            (
                GamePhase::GameOver,
                GameAction::Initialize {
                    player_count,
                    minigames,
                    random_seed,
                },
            ) => {
                if minigames.is_empty() {
                    return Err(anyhow!("Minigames cannot be empty"));
                }
                self.reset(
                    player_count,
                    minigames.into_iter().map(|x| x.into()).collect::<Vec<_>>(),
                    random_seed,
                );
                self.phase = GamePhase::Registration;
                events.push(GameEvent::GameInitialized {
                    player_count,
                    random_seed,
                });
            }

            // Registration Phase
            (GamePhase::Registration, GameAction::RegisterPlayer { name }) => {
                if self.players.len() >= self.max_players {
                    return Err(anyhow!("Game is full"));
                }

                // Check if player already exists by public key
                if self.is_registered(caller) {
                    return Err(anyhow!("Player with identity {} already exists", caller));
                }

                // Check if player already exists by name
                if self.players.iter().any(|p| p.name == name) {
                    return Err(anyhow!("Player with name {} already exists", name));
                }

                self.players.push(Player {
                    id: caller.clone(),
                    name: name.clone(),
                    position: 0,
                    coins: 100,
                    used_uuids: Vec::new(),
                });

                events.push(GameEvent::PlayerRegistered {
                    name: name.clone(),
                    player_id: caller.clone(),
                });
            }

            // Start Game Action
            (GamePhase::Registration, GameAction::StartGame) => {
                let is_full = self.players.len() == self.max_players;
                let registration_period_done =
                    self.last_interaction_time.saturating_add(2 * 60 * 1000) < timestamp;
                if !is_full && !registration_period_done {
                    return Err(anyhow!(
                        "Game is not full and registration period is not over"
                    ));
                }

                self.phase = GamePhase::Betting;
                events.push(GameEvent::GameStarted);
            }

            // Betting Phase
            (GamePhase::Betting, GameAction::PlaceBet { amount }) => {
                if self.bets.contains_key(caller) {
                    return Err(anyhow!("Player has already placed a bet"));
                }
                let Some(player) = self.players.iter().find(|p| p.id == *caller) else {
                    return Err(anyhow!("Player {} not found", caller));
                };
                if self.all_or_nothing {
                    if amount != player.coins as u64 {
                        return Err(anyhow!("All or nothing round: you must bet all your coins"));
                    }
                } else if player.coins < amount as i32 {
                    return Err(anyhow!("Player {} does not have enough coins", caller));
                }
                self.bets.insert(caller.clone(), amount);
                events.push(GameEvent::BetPlaced {
                    player_id: caller.clone(),
                    amount,
                });
                // TODO timeouts
                if self.bets.len() == self.players.len() {
                    if self.round >= ROUNDS {
                        let Some(final_minigame) = self.minigames.first() else {
                            return Err(anyhow!("No final minigame available"));
                        };
                        self.phase = GamePhase::FinalMinigame(final_minigame.clone());
                    } else {
                        self.phase = GamePhase::WheelSpin;
                    }
                    self.all_or_nothing = false; // Reset after round
                } else {
                    self.phase = GamePhase::Betting;
                }
            }

            // Wheel Spin Phase
            (GamePhase::WheelSpin, GameAction::SpinWheel) => {
                // Use dice to determine the wheel outcome
                let outcome = self.dice.roll() % 6;
                events.push(GameEvent::WheelSpun { outcome });
                match outcome {
                    0 => {
                        // Nothing happens, go to next round
                        self.round += 1;
                        self.bets.clear();
                        self.phase = GamePhase::Betting;
                    }
                    1 => {
                        // Randomly pay out the bets to players
                        let bet_entries: Vec<_> =
                            std::mem::take(&mut self.bets).into_iter().collect();
                        let mut player_indices: Vec<_> = (0..self.players.len()).collect();
                        self.dice.shuffle(&mut player_indices);
                        for (i, (bettor, amount)) in bet_entries.iter().enumerate() {
                            // Remove bet from bettor
                            let Some(bettor_idx) =
                                self.players.iter().position(|p| p.id == *bettor)
                            else {
                                return Err(anyhow!("Bettor not found"));
                            };
                            self.update_player_coins(bettor_idx, -(*amount as i32), &mut events)?;
                            // Pay out to a random player
                            let winner_idx = player_indices[i % player_indices.len()];
                            self.update_player_coins(winner_idx, *amount as i32, &mut events)?;
                        }
                        self.round += 1;
                        self.bets.clear();
                        self.phase = GamePhase::Betting;
                    }
                    2 => {
                        // All or nothing: players must bet all their coins next round
                        self.all_or_nothing = true;
                        events.push(GameEvent::AllOrNothingActivated);
                        self.round += 1;
                        self.bets.clear();
                        self.phase = GamePhase::Betting;
                    }
                    _ => {
                        // Minigame: emit MinigameReady and transition to InMinigame for StartMinigame
                        if let Some(minigame_type) = self.minigames.first() {
                            events.push(GameEvent::MinigameReady {
                                minigame_type: minigame_type.0.clone(),
                            });
                            self.phase = GamePhase::StartMinigame(minigame_type.clone());
                        } else {
                            // TODO: should be impossible
                            return Err(anyhow!("No minigame available"));
                        }
                    }
                }
            }

            (
                GamePhase::StartMinigame(expected_minigame),
                GameAction::StartMinigame { minigame, players },
            ) => {
                // Check the starting state is valid.
                if expected_minigame != minigame {
                    return Err(anyhow!("Minigame mismatch"));
                }
                let minigame_players = self.get_minigame_setup();
                if minigame_players != players {
                    return Err(anyhow!("Minigame players mismatch"));
                }
                events.push(GameEvent::MinigameStarted {
                    minigame_type: minigame.0.clone(),
                });
                self.phase = GamePhase::InMinigame(minigame);
            }

            (
                GamePhase::FinalMinigame(final_minigame),
                GameAction::StartMinigame { minigame, players },
            ) => {
                // Check the starting state is valid.
                if minigame == final_minigame {
                    return Err(anyhow!("Minigame mismatch"));
                }
                let minigame_players = self.get_minigame_setup();
                if minigame_players != players {
                    return Err(anyhow!("Minigame players mismatch"));
                }
                events.push(GameEvent::MinigameStarted {
                    minigame_type: minigame.0.clone(),
                });
                self.phase = GamePhase::InMinigame(minigame);
            }

            // InMinigame Phase
            (GamePhase::InMinigame(_), GameAction::EndMinigame { result }) => {
                // Apply results for each player
                for player_result in &result.player_results {
                    self.apply_minigame_result(
                        self.players
                            .iter()
                            .position(|p| p.id == player_result.player_id)
                            .ok_or_else(|| anyhow!("Player not found for minigame result"))?,
                        player_result,
                        &mut events,
                    )?;
                }

                events.push(GameEvent::MinigameEnded { result });

                // End the game if the round limit is reached
                if self.round >= ROUNDS {
                    let winner = self
                        .players
                        .iter()
                        .max_by_key(|p| p.coins)
                        .ok_or_else(|| anyhow!("No players found"))?;
                    events.push(GameEvent::GameEnded {
                        winner_id: winner.id.clone(),
                        final_coins: winner.coins,
                    });
                    self.phase = GamePhase::GameOver;
                } else {
                    self.round += 1;
                    self.bets.clear();
                    self.phase = GamePhase::Betting;
                }
            }

            // Invalid phase/action combinations
            (phase, action) => {
                return Err(anyhow!("Invalid action {:?} for phase {:?}", action, phase));
            }
        }

        Ok(events)
    }

    // Helper function for ending turns and transitioning to rolling phase
    fn end_turn(&mut self, events: &mut Vec<GameEvent>) {
        self.advance_turn();
        let next_player_id = self.get_current_player().unwrap().id.clone();
        events.push(GameEvent::TurnEnded {
            next_player: next_player_id,
        });
        self.phase = GamePhase::Betting;
    }
}
