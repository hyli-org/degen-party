mod metadata {
    use sp1_sdk::include_elf;

    pub const BOARD_GAME_ELF: &[u8] = include_elf!("board_game");
    pub const CRASH_GAME_ELF: &[u8] = include_elf!("crash_game");
}

pub use metadata::*;
