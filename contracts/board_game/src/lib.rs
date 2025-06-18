pub mod game;

use borsh::{BorshDeserialize, BorshSerialize};
use game::{GameAction, GamePhase, GameState};
use sdk::{
    utils::parse_calldata, Blob, BlobData, BlobIndex, Calldata, ContractAction, ContractName,
    Identity, LaneId, RunResult, StateCommitment, StructuredBlobData, ZkContract,
};
use serde::{Deserialize, Serialize};
use smt_token::SmtTokenAction;

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

fn check_blob_in_calldata(
    calldata: &Calldata,
    contract_name: ContractName,
    action: SmtTokenAction,
) -> Result<(), String> {
    for (_, check_blob) in calldata.blobs.iter() {
        if check_blob.contract_name != contract_name {
            continue;
        };
        let Ok(blob) = sdk::StructuredBlob::<SmtTokenAction>::try_from(check_blob.clone()) else {
            continue;
        };
        if blob.data.parameters == action {
            return Ok(());
        }
    }
    Err("Action not found in calldata".into())
}

impl sdk::FullStateRevert for GameState {}

impl ZkContract for GameState {
    fn execute(&mut self, contract_input: &Calldata) -> RunResult {
        let (action, exec_ctx) =
            parse_calldata::<GameActionBlob>(contract_input).map_err(|e| e.to_string())?;

        // For Minigame actions, verify the caller matches the minigame contract
        // The data is validated when processing the action, and is only repeated here
        // so the minigame can use that as a source of truth for composition.
        if let GameAction::StartMinigame { .. } = &action.1 {
            if let GamePhase::StartMinigame(minigame) = &self.phase {
                // Verify that the caller matches the minigame contract name
                if exec_ctx.caller.0 != minigame.0 {
                    return Err("Invalid caller for StartMinigame action".into());
                }
            } else if let GamePhase::FinalMinigame(minigame) = &self.phase {
                if exec_ctx.caller.0 != minigame.0 {
                    return Err("Invalid caller for FinalMinigame action".into());
                }
            } else {
                return Err("Invalid phase for StartMinigame action".into());
            }
        } else if let GameAction::EndMinigame { result } = &action.1 {
            // Verify that the caller matches the minigame contract name
            if exec_ctx.caller.0 != result.contract_name.0 {
                return Err("Invalid caller for EndMinigame action".into());
            }
        } else if let GameAction::RegisterPlayer { deposit, .. } = &action.1 {
            // Ensure player is depositing the correct amount of coins
            check_blob_in_calldata(
                contract_input,
                ContractName::new("oranj"),
                SmtTokenAction::Transfer {
                    sender: contract_input.identity.clone(),
                    recipient: Identity::new(exec_ctx.contract_name.clone().0),
                    amount: *deposit as u128,
                },
            )?;
        } else if let GameAction::DistributeRewards = &action.1 {
            // Check that we have a transfer blob for all players
            for player in &self.players {
                check_blob_in_calldata(
                    contract_input,
                    ContractName::new("oxygen"),
                    SmtTokenAction::Transfer {
                        sender: Identity::new(exec_ctx.contract_name.clone().0),
                        recipient: player.id.clone(),
                        amount: player.coins as u128,
                    },
                )?;
            }
        }

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

        Ok((borsh::to_vec(&events).unwrap(), exec_ctx, vec![]))
    }

    fn commit(&self) -> StateCommitment {
        StateCommitment(borsh::to_vec(self).unwrap())
    }
}
