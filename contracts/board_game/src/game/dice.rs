use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct Dice {
    min: u8,
    max: u8,
    pub seed: u64,
}

impl Default for Dice {
    fn default() -> Self {
        Self::new(1, 10, 12345)
    }
}

impl Dice {
    pub fn new(min: u8, max: u8, seed: u64) -> Self {
        assert!(min < max, "Minimum value must be less than maximum value");
        Self { min, max, seed }
    }

    pub fn roll(&mut self) -> u8 {
        // Simple Linear Congruential Generator
        self.seed = self.seed.wrapping_mul(1103515245).wrapping_add(12345);
        let range = (self.max - self.min + 1) as u64;
        self.min + ((self.seed >> 16) % range) as u8
    }
}
