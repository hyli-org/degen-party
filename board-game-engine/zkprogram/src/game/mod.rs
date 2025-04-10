use anyhow::{anyhow, bail, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use hyle_contract_sdk::{ContractName, Identity, StateCommitment};
use serde::{Deserialize, Serialize};

pub mod board;
pub mod dice;
pub mod player;
pub mod utils;

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GameState {
    pub players: Vec<Player>,
    pub current_turn: usize,
    pub board: Board,
    pub phase: GamePhase,
    pub max_players: usize,
    pub minigames: Vec<ContractName>,
    pub dice: dice::Dice,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Player {
    pub id: Identity,
    pub name: String,
    pub position: usize,
    pub coins: i32,
    pub stars: i32,
    pub used_uuids: Vec<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Board {
    pub spaces: Vec<Space>,
    pub size: usize,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub enum Space {
    Blue,
    Red,
    Event,
    MinigameSpace,
    Star,
    Finish,
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
    pub stars_delta: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum GamePhase {
    Registration,
    Rolling,
    Moving,
    MinigameStart(ContractName),
    MinigamePlay(ContractName),
    TurnEnd,
    GameOver,
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
pub enum GameAction {
    RegisterPlayer { name: String, identity: Identity },
    StartGame,
    RollDice,
    StartMinigame,
    EndMinigame { result: MinigameResult },
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
    StarsChanged {
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
        final_stars: i32,
        final_coins: i32,
    },
    PlayerRegistered {
        name: String,
        player_id: Identity,
    },
    GameStarted,
}

impl From<StateCommitment> for GameState {
    fn from(state: StateCommitment) -> Self {
        GameState::try_from_slice(&state.0).unwrap()
    }
}

impl GameState {
    pub fn new(player_count: usize, board_size: usize, random_seed: u64) -> Self {
        Self {
            players: Vec::with_capacity(player_count),
            current_turn: 0,
            board: Board::new(board_size, random_seed),
            phase: GamePhase::Registration,
            max_players: player_count,
            minigames: vec!["crash_game".into()],
            dice: dice::Dice::new(1, 10, random_seed),
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

    // Helper function for updating stars and generating events
    fn update_player_stars(
        &mut self,
        player_index: usize,
        delta: i32,
        events: &mut Vec<GameEvent>,
    ) -> Result<()> {
        let Some(player) = self.players.get_mut(player_index) else {
            return Err(anyhow!("Player not found"));
        };
        player.stars = (player.stars + delta).max(0);
        events.push(GameEvent::StarsChanged {
            player_id: player.id.clone(),
            amount: delta,
        });
        Ok(())
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

        if result.stars_delta != 0 {
            self.update_player_stars(player_index, result.stars_delta, events)?;
        }
        Ok(())
    }

    fn get_current_player_index(&self) -> usize {
        self.current_turn % self.players.len()
    }

    fn get_current_player(&self) -> Result<&Player> {
        self.players
            .get(self.current_turn % self.players.len())
            .ok_or_else(|| anyhow!("Invalid current turn index"))
    }

    fn get_current_player_mut(&mut self) -> Result<&mut Player> {
        let len = self.players.len();
        self.players
            .get_mut(self.current_turn % len)
            .ok_or_else(|| anyhow!("Invalid current turn index"))
    }

    fn advance_turn(&mut self) {
        self.current_turn += 1;
    }

    fn determine_winner(&self) -> (Identity, i32, i32) {
        let winner = self
            .players
            .iter()
            .max_by(|a, b| {
                // First compare stars
                let star_cmp = a.stars.cmp(&b.stars);
                if star_cmp != std::cmp::Ordering::Equal {
                    return star_cmp;
                }
                // If stars are equal, compare coins
                a.coins.cmp(&b.coins)
            })
            .unwrap();

        (winner.id.clone(), winner.stars, winner.coins)
    }

    pub fn process_action(
        &mut self,
        caller: &Identity,
        uuid: u128,
        action: GameAction,
    ) -> Result<Vec<GameEvent>> {
        let mut events = Vec::new();

        if let Some(player) = self.players.iter_mut().find(|p| &p.id == caller) {
            if player.used_uuids.contains(&uuid) {
                bail!("UUID already used");
            }
            player.used_uuids.push(uuid);
        }

        match (self.phase.clone(), action) {
            // Registration Phase
            (GamePhase::Registration, GameAction::RegisterPlayer { name, identity }) => {
                if self.players.len() >= self.max_players {
                    return Err(anyhow!("Game is full"));
                }

                // Check if player already exists by public key
                if self.players.iter().any(|p| p.id == identity) {
                    return Err(anyhow!(
                        "Player with public key {} already exists",
                        identity
                    ));
                }
                // Check if player already exists by name
                if self.players.iter().any(|p| p.name == name) {
                    return Err(anyhow!("Player with name {} already exists", name));
                }

                self.players.push(Player {
                    id: identity.clone(),
                    name: name.clone(),
                    position: 0,
                    coins: 100,
                    stars: 0,
                    used_uuids: Vec::new(),
                });

                events.push(GameEvent::PlayerRegistered {
                    name: name.clone(),
                    player_id: identity,
                });
            }

            // Start Game Action
            (GamePhase::Registration, GameAction::StartGame) => {
                //if self.players.len() < 2 {
                //    return Err(anyhow!("Need at least 2 players to start the game"));
                //}
                self.phase = GamePhase::Rolling;
                events.push(GameEvent::GameStarted);
            }

            // Rolling Phase
            (GamePhase::Rolling, GameAction::RollDice) => {
                let current_player = self.get_current_player()?.clone();
                let roll_value = self.dice.roll();

                events.push(GameEvent::DiceRolled {
                    player_id: current_player.id,
                    value: roll_value,
                });

                // Move to movement phase
                self.phase = GamePhase::Moving;

                // Automatically handle movement
                let new_position = board::calculate_next_position(
                    current_player.position,
                    roll_value as i32,
                    self.board.size,
                );

                {
                    let current_player = self.get_current_player_mut()?;
                    current_player.position = new_position;
                }
                let current_player = self.get_current_player()?.clone();

                events.push(GameEvent::PlayerMoved {
                    player_id: current_player.id,
                    new_position,
                });

                let space = *self
                    .board
                    .spaces
                    .get(current_player.position)
                    .ok_or_else(|| anyhow!("Invalid player position"))?;

                let player_index = self.get_current_player_index();
                match space {
                    Space::Blue => {
                        self.update_player_coins(player_index, 3, &mut events)?;
                    }
                    Space::Red => {
                        if current_player.coins >= 3 {
                            self.update_player_coins(player_index, -3, &mut events)?;
                        } else {
                            let current_coins = current_player.coins;
                            self.update_player_coins(player_index, -current_coins, &mut events)?;
                        }
                    }
                    Space::Star => {
                        if current_player.coins >= 20 {
                            self.update_player_coins(player_index, -20, &mut events)?;
                            self.update_player_stars(player_index, 1, &mut events)?;
                        }
                    }
                    Space::MinigameSpace => {
                        if let Some(minigame_type) = self.minigames.first() {
                            self.phase = GamePhase::MinigameStart(minigame_type.clone());
                            events.push(GameEvent::MinigameReady {
                                minigame_type: minigame_type.clone().0,
                            });
                            return Ok(events);
                        } else {
                            return Err(anyhow!("No minigames available"));
                        }
                    }
                    Space::Event => {
                        // For now, events just give or take a random amount of coins (-5 to +5)
                        let roll = self.dice.roll() as i32;
                        let coin_change = if roll % 2 == 0 { roll } else { -roll };
                        self.update_player_coins(player_index, coin_change, &mut events)?;
                    }
                    Space::Finish => {
                        // Game is over, determine the winner
                        let (winner_id, final_stars, final_coins) = self.determine_winner();
                        events.push(GameEvent::GameEnded {
                            winner_id,
                            final_stars,
                            final_coins,
                        });
                        self.phase = GamePhase::GameOver;
                        return Ok(events);
                    }
                }

                if self.phase != GamePhase::GameOver {
                    self.phase = GamePhase::TurnEnd;
                    self.end_turn(&mut events);
                }
            }

            // Minigame Setup Phase
            (GamePhase::MinigameStart(minigame_type), GameAction::StartMinigame) => {
                events.push(GameEvent::MinigameStarted {
                    minigame_type: minigame_type.0.clone(),
                });
                self.phase = GamePhase::MinigamePlay(minigame_type.clone());
            }

            // Minigame End Phase
            (GamePhase::MinigamePlay(minigame_type), GameAction::EndMinigame { result }) => {
                // Verify the minigame contract is valid
                if minigame_type != result.contract_name {
                    return Err(anyhow!("Invalid minigame contract"));
                }

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

                self.phase = GamePhase::TurnEnd;
                self.end_turn(&mut events);
            }

            // Turn End Phase
            (GamePhase::TurnEnd, GameAction::EndTurn) => {
                self.end_turn(&mut events);
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
        self.phase = GamePhase::Rolling;
    }
}
