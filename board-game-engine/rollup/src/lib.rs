pub mod crash_game;
pub mod fake_lane_manager;
pub mod game_state;
pub mod websocket;

use hyle::bus::SharedMessageBus;
pub struct Context {
    pub bus: SharedMessageBus,
}
