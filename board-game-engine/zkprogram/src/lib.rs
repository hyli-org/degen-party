pub mod game;

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use game::{GameAction, GameEvent, GameState};
use hyle_contract_sdk::{
    guest::fail, info, utils::parse_calldata, Blob, BlobData, BlobIndex, Calldata, ContractAction,
    ContractName, RunResult, StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};

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

impl ZkContract for GameState {
    fn execute(&mut self, contract_input: &Calldata) -> RunResult {
        let (action, exec_ctx) =
            parse_calldata::<GameActionBlob>(contract_input).map_err(|e| e.to_string())?;

        info!(
            "Executing action: {:?} with caller: {:?}",
            action.1, exec_ctx.caller
        );

        // For EndMinigame actions, verify the caller matches the minigame contract
        if let GameAction::EndMinigame { result } = &action.1 {
            // Verify that the caller matches the minigame contract name
            if exec_ctx.caller.0 != result.contract_name.0 {
                fail(
                    contract_input,
                    self.commit(),
                    "Invalid caller for EndMinigame action",
                );
            }
        } else if let GameAction::StartMinigame = &action.1 {
            // Check that we're calling the minigame with an approriate blob
            let game::GamePhase::MinigameStart(contract_name) = &self.phase else {
                return Err("Invalid game phase for StartMinigame action".into());
            };
            // Check that one of the other blobs is for the minigame, but this doesn't really do much TBH.
            if contract_input
                .blobs
                .iter()
                .all(|blob| blob.contract_name != *contract_name)
            {
                fail(
                    contract_input,
                    self.commit(),
                    "No blob to actually start the minigame seem present",
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
