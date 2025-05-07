use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub position: usize,
    pub coins: i32,
    pub stars: i32,
    pub items: Vec<Item>,
    pub status_effects: Vec<StatusEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Item {
    CustomDice(u8), // Allows rolling a specific number
    CoinMultiplier, // Doubles coin gains/losses
    StarSteal,      // Allows stealing a star from another player
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatusEffect {
    SkipNextTurn,
    ReverseMovement,
    CoinBonus,
    CoinPenalty,
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

    pub fn add_stars(&mut self, amount: i32) -> Result<i32> {
        let new_amount = self
            .stars
            .checked_add(amount)
            .ok_or_else(|| anyhow!("Star overflow"))?;

        if new_amount < 0 {
            self.stars = 0;
            Ok(self.stars)
        } else {
            self.stars = new_amount;
            Ok(self.stars)
        }
    }

    pub fn add_item(&mut self, item: Item) -> Result<()> {
        if self.items.len() >= 3 {
            return Err(anyhow!("Item inventory full"));
        }
        self.items.push(item);
        Ok(())
    }

    pub fn remove_item(&mut self, index: usize) -> Result<Item> {
        if index >= self.items.len() {
            return Err(anyhow!("Invalid item index"));
        }
        Ok(self.items.remove(index))
    }

    pub fn add_status_effect(&mut self, effect: StatusEffect) {
        self.status_effects.push(effect);
    }

    pub fn clear_status_effects(&mut self) {
        self.status_effects.clear();
    }

    pub fn has_status_effect(&self, effect: &StatusEffect) -> bool {
        self.status_effects
            .iter()
            .any(|e| std::mem::discriminant(e) == std::mem::discriminant(effect))
    }
}
