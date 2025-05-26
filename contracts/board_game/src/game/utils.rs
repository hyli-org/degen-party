use super::GameEvent;

impl std::fmt::Display for GameEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameEvent::DiceRolled { player_id, value } => {
                write!(f, "Player {} rolled a {}", player_id, value)
            }
            GameEvent::PlayerMoved {
                player_id,
                new_position,
            } => {
                write!(f, "Player {} moved to position {}", player_id, new_position)
            }
            GameEvent::CoinsChanged { player_id, amount } => {
                if *amount > 0 {
                    write!(f, "Player {} gained {} coins", player_id, amount)
                } else {
                    write!(f, "Player {} lost {} coins", player_id, amount.abs())
                }
            }
            GameEvent::BetPlaced { player_id, amount } => {
                write!(f, "Player {} placed a bet of {}", player_id, amount)
            }
            GameEvent::WheelSpun { round, outcome } => {
                write!(f, "Wheel spun for round {}, outcome: {}", round, outcome)
            }
            GameEvent::MinigameReady { minigame_type } => {
                write!(f, "Minigame '{}' is ready", minigame_type)
            }
            GameEvent::MinigameStarted { minigame_type } => {
                write!(f, "Minigame '{}' started", minigame_type)
            }
            GameEvent::MinigameEnded { result } => {
                write!(f, "Minigame ended with result: {:?}", result)
            }
            GameEvent::TurnEnded { next_player } => {
                write!(f, "Turn ended, next player is {}", next_player)
            }
            GameEvent::GameEnded {
                winner_id,
                final_coins,
            } => {
                write!(
                    f,
                    "Game ended, winner is {}, final coins: {}",
                    winner_id, final_coins
                )
            }
            GameEvent::GameInitialized { random_seed } => {
                write!(f, "Game initialized, random seed {}", random_seed)
            }
            GameEvent::PlayerRegistered { name, player_id } => {
                write!(f, "Player {} registered as {}", name, player_id)
            }
            GameEvent::GameStarted { player_count } => {
                write!(f, "Game started with {} players", player_count)
            }
            _ => {
                write!(f, "Unknown game event")
            }
        }
    }
}
