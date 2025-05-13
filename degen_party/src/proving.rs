use std::sync::Arc;

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use client_sdk::transaction_builder::TxExecutorHandler;
use hyle_modules::modules::{
    prover::{AutoProver, AutoProverCtx},
    ModulesHandler,
};
use sdk::{utils::as_hyle_output, BlockHeight, ZkContract};

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct BoardGameExecutor {
    state: board_game::game::GameState,
}

impl TxExecutorHandler for BoardGameExecutor {
    fn handle(&mut self, calldata: &sdk::Calldata) -> Result<sdk::HyleOutput, String> {
        let initial_state_commitment = self.state.commit();
        let mut res = self.state.execute(calldata);
        Ok(as_hyle_output(
            initial_state_commitment,
            self.state.commit(),
            calldata,
            &mut res,
        ))
    }

    fn build_commitment_metadata(&self, _blob: &sdk::Blob) -> Result<Vec<u8>, String> {
        Ok(self.state.commit().0)
    }
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct CrashGameExecutor {
    state: crash_game::GameState,
}

impl TxExecutorHandler for CrashGameExecutor {
    fn handle(&mut self, calldata: &sdk::Calldata) -> Result<sdk::HyleOutput, String> {
        let initial_state_commitment = self.state.commit();
        let mut res = self.state.execute(calldata);
        Ok(as_hyle_output(
            initial_state_commitment,
            self.state.commit(),
            calldata,
            &mut res,
        ))
    }

    fn build_commitment_metadata(&self, _blob: &sdk::Blob) -> Result<Vec<u8>, String> {
        Ok(self.state.commit().0)
    }
}

pub async fn setup_auto_provers(
    ctx: Arc<crate::Context>,
    handler: &mut ModulesHandler,
) -> Result<()> {
    #[cfg(not(feature = "fake_proofs"))]
    let board_game_prover = Arc::new(client_sdk::helpers::sp1::SP1Prover::new(
        contracts::BOARD_GAME_ELF,
    ));
    #[cfg(feature = "fake_proofs")]
    let board_game_prover = Arc::new(client_sdk::helpers::test::TxExecutorTestProver::new(
        BoardGameExecutor::default(),
    ));

    handler
        .build_module::<AutoProver<BoardGameExecutor>>(
            AutoProverCtx {
                data_directory: ctx.data_directory.clone(),
                start_height: BlockHeight(0),
                prover: board_game_prover,
                contract_name: "board_game".into(),
                node: ctx.client.clone(),
            }
            .into(),
        )
        .await?;

    #[cfg(not(feature = "fake_proofs"))]
    let crash_game_prover = Arc::new(client_sdk::helpers::sp1::SP1Prover::new(
        contracts::CRASH_GAME_ELF,
    ));
    #[cfg(feature = "fake_proofs")]
    let crash_game_prover = Arc::new(client_sdk::helpers::test::TxExecutorTestProver::new(
        CrashGameExecutor {
            state: borsh::from_slice(&{
                // We expect the game to not be running, so we go through the default init path
                // Skip the last 4 bytes
                let mut state = ctx.client.get_contract(&"crash_game".into()).await?.state.0;
                state.pop();
                state.pop();
                state.pop();
                state.pop();
                state
            })?,
        },
    ));

    handler
        .build_module::<AutoProver<CrashGameExecutor>>(
            AutoProverCtx {
                data_directory: ctx.data_directory.clone(),
                start_height: BlockHeight(0),
                prover: crash_game_prover,
                contract_name: "crash_game".into(),
                node: ctx.client.clone(),
            }
            .into(),
        )
        .await?;

    Ok(())
}
