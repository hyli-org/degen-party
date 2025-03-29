use rand::Rng;

pub struct Dice {
    min: u8,
    max: u8,
}

impl Default for Dice {
    fn default() -> Self {
        Self::new(1, 10)
    }
}

impl Dice {
    pub fn new(min: u8, max: u8) -> Self {
        assert!(min < max, "Minimum value must be less than maximum value");
        Self { min, max }
    }

    pub fn roll(&self) -> u8 {
        rand::thread_rng().gen_range(self.min..=self.max)
    }
}
