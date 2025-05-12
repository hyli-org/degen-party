use super::{dice::Dice, Board, Space};

pub struct BoardBuilder {
    size: usize,
    blue_ratio: f32,
    red_ratio: f32,
    event_ratio: f32,
    minigame_ratio: f32,
    star_ratio: f32,
    seed: u64,
}

impl Default for BoardBuilder {
    fn default() -> Self {
        Self {
            size: 50,
            blue_ratio: 0.4,
            red_ratio: 0.2,
            event_ratio: 0.2,
            minigame_ratio: 0.15,
            star_ratio: 0.05,
            seed: 12345,
        }
    }
}

impl BoardBuilder {
    pub fn new(size: usize, seed: u64) -> Self {
        Self {
            size,
            seed,
            ..Default::default()
        }
    }

    pub fn with_ratios(
        mut self,
        blue: f32,
        red: f32,
        event: f32,
        minigame: f32,
        star: f32,
    ) -> Self {
        let total = blue + red + event + minigame + star;
        assert!((total - 1.0).abs() < f32::EPSILON, "Ratios must sum to 1.0");

        self.blue_ratio = blue;
        self.red_ratio = red;
        self.event_ratio = event;
        self.minigame_ratio = minigame;
        self.star_ratio = star;
        self
    }

    pub fn build(self) -> Vec<Space> {
        let actual_size = self.size - 1; // Reserve one space for the finish
        let mut spaces = Vec::with_capacity(self.size);

        let blue_count = (actual_size as f32 * self.blue_ratio).round() as usize;
        let red_count = (actual_size as f32 * self.red_ratio).round() as usize;
        let event_count = (actual_size as f32 * self.event_ratio).round() as usize;
        let minigame_count = (actual_size as f32 * self.minigame_ratio).round() as usize;
        let star_count = (actual_size as f32 * self.star_ratio).round() as usize;

        spaces.extend(vec![Space::Blue; blue_count]);
        spaces.extend(vec![Space::Red; red_count]);
        spaces.extend(vec![Space::Event; event_count]);
        spaces.extend(vec![Space::MinigameSpace; minigame_count]);
        spaces.extend(vec![Space::Star; star_count]);

        // Pad with blue spaces if we're short
        while spaces.len() < actual_size {
            spaces.push(Space::Blue);
        }

        // Truncate if we're over due to rounding
        spaces.truncate(actual_size);

        // Shuffle all spaces except the last one using our deterministic RNG
        let mut dice = Dice::new(0, actual_size as u8 - 1, self.seed);
        for i in (1..actual_size).rev() {
            let j = dice.roll() as usize;
            if j < i {
                spaces.swap(i, j);
            }
        }

        // Add the finish space at the end
        spaces.push(Space::Finish);

        spaces
    }
}

pub fn calculate_next_position(current: usize, movement: i32, board_size: usize) -> usize {
    let board_size = board_size as i32;
    let current = current as i32;

    let raw_position = current + movement;
    let mut final_position = raw_position % board_size;

    // Handle negative movement
    if final_position < 0 {
        final_position += board_size;
    }

    final_position as usize
}

impl Board {
    pub fn new(size: usize, seed: u64) -> Self {
        Self {
            spaces: BoardBuilder::new(size, seed).build(),
            size,
        }
    }

    pub fn with_custom_ratios(
        size: usize,
        seed: u64,
        blue: f32,
        red: f32,
        event: f32,
        minigame: f32,
        star: f32,
    ) -> Self {
        Self {
            spaces: BoardBuilder::new(size, seed)
                .with_ratios(blue, red, event, minigame, star)
                .build(),
            size,
        }
    }
}
