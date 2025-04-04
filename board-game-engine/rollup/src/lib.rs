use anyhow::Result;
use board_game_engine::game::{GameAction, GameEvent, GameState};
use borsh::{BorshDeserialize, BorshSerialize};
use hyle::bus::SharedMessageBus;
use hyle_contract_sdk::{ContractInput, HyleContract, RunResult, StateCommitment};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod crash_game;
pub mod fake_lane_manager;
pub mod game_state;
pub mod websocket;

pub struct Context {
    pub bus: SharedMessageBus,
}
