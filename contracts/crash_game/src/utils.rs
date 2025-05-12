use core::fmt;

use crate::ChainEvent;

impl fmt::Display for ChainEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainEvent::MinigameInitialized { player_count } => {
                write!(f, "Minigame initialized with {} players", player_count)
            }
            ChainEvent::BetPlaced { player_id, amount } => {
                write!(f, "Player {} placed bet of {}", player_id, amount)
            }
            ChainEvent::GameStarted => {
                write!(f, "Game started")
            }
            ChainEvent::PlayerCashedOut {
                player_id,
                multiplier,
                winnings,
            } => {
                write!(
                    f,
                    "Player {} cashed out at {}x and won {}",
                    player_id, multiplier, winnings
                )
            }
            ChainEvent::GameCrashed { final_multiplier } => {
                write!(f, "Game crashed at {}x", final_multiplier)
            }
            ChainEvent::MinigameEnded { final_results } => {
                write!(
                    f,
                    "Minigame ended with {} player results",
                    final_results.len()
                )
            }
        }
    }
}
