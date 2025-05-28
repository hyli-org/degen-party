use std::{
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use client_sdk::transaction_builder::TxExecutorHandler;
use crash_game::{ChainAction, ChainActionBlob};
use hyle_modules::modules::{
    prover::{AutoProver, AutoProverCtx},
    ModulesHandler,
};
use sdk::{
    utils::as_hyle_output, Identity, RegisterContractEffect, StructuredBlobData, ZkContract,
};
use sp1_sdk::Prover;
use sp1_sdk::SP1ProvingKey;
use tracing::info;

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct BoardGameExecutor {
    pub state: board_game::game::GameState,
}

impl TxExecutorHandler for BoardGameExecutor {
    fn handle(&mut self, calldata: &sdk::Calldata) -> Result<sdk::HyleOutput> {
        let initial_state_commitment = self.state.commit();
        let mut res = self.state.execute(calldata);
        Ok(as_hyle_output(
            initial_state_commitment,
            self.state.commit(),
            calldata,
            &mut res,
        ))
    }

    fn build_commitment_metadata(&self, _blob: &sdk::Blob) -> Result<Vec<u8>> {
        Ok(self.state.commit().0)
    }

    fn construct_state(
        _register_blob: &RegisterContractEffect,
        metadata: &Option<Vec<u8>>,
    ) -> anyhow::Result<Self> {
        if let Some(metadata) = metadata {
            Ok(Self {
                state: board_game::game::GameState::new(borsh::from_slice(metadata)?),
            })
        } else {
            anyhow::bail!("No metadata provided");
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct CrashGameExecutor {
    pub state: crash_game::GameState,
}

impl TxExecutorHandler for CrashGameExecutor {
    fn handle(&mut self, calldata: &sdk::Calldata) -> Result<sdk::HyleOutput> {
        let initial_state_commitment = self.state.commit();
        let mut res = self.state.execute(calldata);

        if let Ok(StructuredBlobData::<ChainActionBlob> { parameters, .. }) =
            StructuredBlobData::<ChainActionBlob>::try_from(
                calldata.blobs.get(&calldata.index).unwrap().data.clone(),
            )
        {
            tracing::warn!("Received ChainActionBlob: {:?}", parameters);
            if let ChainAction::InitMinigame { .. } = parameters.1 {
                /*let mut state = GameState::new(
                    self.board_game.clone(),
                    Identity::new(format!("{}@secp256k1", self.crypto.public_key)),
                );
                */
                self.state.minigame_backend.game_setup_time =
                    Some(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis());
                self.state.minigame_backend.current_time =
                    self.state.minigame_backend.game_setup_time;
            } else if let ChainAction::Start = parameters.1 {
                let t = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
                self.state.minigame_backend.game_start_time = Some(t);
                self.state.minigame_backend.current_time = Some(t);
            }
        }

        Ok(as_hyle_output(
            initial_state_commitment,
            self.state.commit(),
            calldata,
            &mut res,
        ))
    }

    fn build_commitment_metadata(&self, _blob: &sdk::Blob) -> Result<Vec<u8>> {
        Ok(self.state.commit().0)
    }

    fn construct_state(
        _register_blob: &RegisterContractEffect,
        metadata: &Option<Vec<u8>>,
    ) -> anyhow::Result<Self> {
        if let Some(metadata) = metadata {
            let (board_contract, backend_identity) = borsh::from_slice(metadata)?;
            Ok(Self {
                state: crash_game::GameState::new(board_contract, backend_identity),
            })
        } else {
            anyhow::bail!("No metadata provided");
        }
    }
}

pub async fn setup_auto_provers(
    ctx: Arc<crate::Context>,
    handler: &mut ModulesHandler,
) -> Result<()> {
    let board_game_executor = BoardGameExecutor {
        state: board_game::game::GameState::new(Identity::new(format!(
            "{}@secp256k1",
            ctx.crypto.public_key
        ))),
    };
    let crash_game_state: crash_game::GameState = crash_game::GameState::new(
        ctx.board_game.clone(),
        Identity::new(format!("{}@secp256k1", ctx.crypto.public_key,)),
    );
    let crash_game_executor = CrashGameExecutor {
        state: crash_game_state,
    };
    #[cfg(not(feature = "fake_proofs"))]
    let board_game_prover = {
        let pk = load_pk(
            contracts::BOARD_GAME_ELF,
            &ctx.data_directory.join("board_game_pk.json"),
        );
        Arc::new(client_sdk::helpers::sp1::SP1Prover::new(pk).await)
    };
    #[cfg(feature = "fake_proofs")]
    let board_game_prover = Arc::new(client_sdk::helpers::test::TxExecutorTestProver::<
        board_game::game::GameState,
    >::new());

    handler
        .build_module::<AutoProver<BoardGameExecutor>>(
            AutoProverCtx {
                data_directory: ctx.data_directory.clone(),
                prover: board_game_prover,
                contract_name: ctx.board_game.clone(),
                node: ctx.client.clone(),
                default_state: board_game_executor,
            }
            .into(),
        )
        .await?;

    #[cfg(not(feature = "fake_proofs"))]
    let crash_game_prover = {
        let pk = load_pk(
            contracts::CRASH_GAME_ELF,
            &ctx.data_directory.join("crash_game_pk.json"),
        );
        Arc::new(client_sdk::helpers::sp1::SP1Prover::new(pk).await)
    };
    #[cfg(feature = "fake_proofs")]
    let crash_game_prover = Arc::new(client_sdk::helpers::test::TxExecutorTestProver::<
        crash_game::GameState,
    >::new());

    handler
        .build_module::<AutoProver<CrashGameExecutor>>(Arc::new(AutoProverCtx {
            data_directory: ctx.data_directory.clone(),
            prover: crash_game_prover,
            contract_name: ctx.crash_game.clone(),
            node: ctx.client.clone(),
            default_state: crash_game_executor,
        }))
        .await?;

    Ok(())
}

pub fn load_pk(elf: &[u8], pk_path: &Path) -> SP1ProvingKey {
    if pk_path.exists() {
        info!("Loading proving key from disk");
        return std::fs::read(pk_path)
            .map(|bytes| serde_json::from_slice(&bytes).expect("Failed to deserialize proving key"))
            .expect("Failed to read proving key from disk");
    } else if let Err(e) = std::fs::create_dir_all(pk_path.parent().unwrap()) {
        tracing::error!("Failed to create data directory: {}", e);
    }

    info!("Building proving key");
    let client = sp1_sdk::ProverClient::builder().cpu().build();
    let (pk, _) = client.setup(elf);

    if let Err(e) = std::fs::write(
        pk_path,
        serde_json::to_vec(&pk).expect("Failed to serialize proving key"),
    ) {
        tracing::error!("Failed to save proving key to disk: {}", e);
    }

    pk
}
