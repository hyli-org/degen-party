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
            GameEvent::StarsChanged { player_id, amount } => {
                if *amount > 0 {
                    write!(f, "Player {} gained {} stars", player_id, amount)
                } else {
                    write!(f, "Player {} lost {} stars", player_id, amount.abs())
                }
            }
            GameEvent::MinigameStarted { minigame_type } => {
                write!(f, "Minigame '{}' started", minigame_type)
            }
            GameEvent::TurnEnded { next_player } => {
                write!(f, "Turn ended, next player is {}", next_player)
            }
            GameEvent::GameEnded {
                winner_id,
                final_stars,
                final_coins,
            } => {
                write!(
                    f,
                    "Game ended, winner is {}, final stars: {}, final coins: {}",
                    winner_id, final_stars, final_coins
                )
            }
            GameEvent::PlayerRegistered { name, player_id } => {
                write!(f, "Player {} registered as {}", name, player_id)
            }
            GameEvent::GameStarted => {
                write!(f, "Game started")
            }
            _ => write!(f, "Unknown event"),
        }
    }
}
