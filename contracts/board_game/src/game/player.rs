use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub position: usize,
    pub coins: i32,
}

impl PlayerState {
    pub fn add_coins(&mut self, amount: i32) -> Result<i32> {
        let new_amount = self
            .coins
            .checked_add(amount)
            .ok_or_else(|| anyhow!("Coin overflow"))?;

        if new_amount < 0 {
            self.coins = 0;
            Ok(self.coins)
        } else {
            self.coins = new_amount;
            Ok(self.coins)
        }
    }
}
