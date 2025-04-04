pub mod game;

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use game::{GameAction, GameEvent, GameState};
use hyle_contract_sdk::{
    guest::fail, utils::parse_contract_input, Blob, BlobData, BlobIndex, ContractAction,
    ContractInput, ContractName, HyleContract, RunResult, StateCommitment, StructuredBlobData,
};
use serde::{Deserialize, Serialize};

/// Creates a new game with the specified number of players and board size
pub fn create_game(player_count: usize, board_size: usize) -> GameState {
    GameState::new(player_count, board_size)
}

/// Process a game action and return the resulting events
pub fn process_game_action(state: &mut GameState, action: GameAction) -> Result<Vec<GameEvent>> {
    state.process_action(action)
}

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
// First string is a UUID just to avoid having the same blob hashes.
pub struct GameActionBlob(pub String, pub GameAction);

impl ContractAction for GameActionBlob {
    fn as_blob(
        &self,
        contract_name: ContractName,
        caller: Option<BlobIndex>,
        callees: Option<Vec<BlobIndex>>,
    ) -> Blob {
        Blob {
            contract_name,
            data: BlobData::from(StructuredBlobData {
                caller,
                callees,
                parameters: self.clone(),
            }),
        }
    }
}

impl HyleContract for GameState {
    fn execute(&mut self, contract_input: &ContractInput) -> RunResult {
        let (action, mut exec_ctx) =
            parse_contract_input::<GameActionBlob>(contract_input).map_err(|e| e.to_string())?;

        // For EndMinigame actions, verify the caller matches the minigame contract
        if let GameAction::EndMinigame { result } = &action.1 {
            // Verify that the caller matches the minigame contract name
            if exec_ctx.caller.0 != result.contract_name.0 {
                fail(
                    contract_input.clone(),
                    self.commit(),
                    "Invalid caller for EndMinigame action",
                );
            }
        }

        let events = self.process_action(action.1).map_err(|e| e.to_string())?;

        let game_events = events
            .iter()
            .map(|event| event.to_string())
            .collect::<Vec<String>>();
        Ok((game_events.join("\n"), exec_ctx, vec![]))
    }

    fn commit(&self) -> StateCommitment {
        StateCommitment(borsh::to_vec(self).unwrap())
    }
}
