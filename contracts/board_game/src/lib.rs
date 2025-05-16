pub mod game;

use borsh::{BorshDeserialize, BorshSerialize};
use game::{GameAction, GameState};
use sdk::{
    secp256k1, utils::parse_calldata, Blob, BlobData, BlobIndex, Calldata, ContractAction,
    ContractName, LaneId, RunResult, StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, BorshSerialize, BorshDeserialize, PartialEq)]
// First string is a UUID just to avoid having the same blob hashes.
pub struct GameActionBlob(pub u128, pub GameAction);

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

        // For EndMinigame actions, verify the caller matches the minigame contract
        if let GameAction::EndMinigame { result } = &action.1 {
            // Verify that the caller matches the minigame contract name
            if exec_ctx.caller.0 != result.contract_name.0 {
                return Err("Invalid caller for EndMinigame action".into());
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
                .all(|(_, blob)| blob.contract_name != *contract_name)
            {
                return Err("Invalid contract name for StartMinigame action".into());
            }
        } else if let GameAction::RegisterPlayer { identity, .. } = &action.1 {
            // Check we are this pubkey
            if &contract_input.identity != identity {
                return Err("Invalid public key for RegisterPlayer action".into());
            }
        }

        let expected_data = uuid::Uuid::from_u128(action.0).to_string();

        let expected_action_data = match &action.1 {
            GameAction::EndGame => "EndGame",
            GameAction::Initialize { .. } => "Initialize",
            GameAction::StartGame => "StartGame",
            GameAction::RegisterPlayer { .. } => "RegisterPlayer",
            GameAction::RollDice => "RollDice",
            GameAction::EndTurn => "EndTurn",
            GameAction::StartMinigame => "StartMinigame",
            GameAction::EndMinigame { .. } => "EndMinigame",
        };

        secp256k1::CheckSecp256k1::new(
            contract_input,
            format!("{}:{}", expected_data, expected_action_data).as_bytes(),
        )
        .expect()?;

        let Some(ref ctx) = contract_input.tx_ctx else {
            return Err("Missing transaction context".into());
        };

        // Rollup mode, ensure everything is sent to the same lane ID or we are well past interaction timeout
        let interaction_timeout = ctx.timestamp.0.saturating_add(60 * 60 * 24 * 1000); // 24 hours
        if self.lane_id == LaneId::default() || ctx.timestamp.0 > interaction_timeout {
            self.lane_id = ctx.lane_id.clone();
        } else if self.lane_id != ctx.lane_id {
            return Err("Invalid lane ID".into());
        }

        let events = self
            .process_action(
                &contract_input.identity,
                action.0,
                action.1,
                ctx.timestamp.0,
            )
            .map_err(|e| e.to_string())?;

        self.last_interaction_time = ctx.timestamp.0;

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
