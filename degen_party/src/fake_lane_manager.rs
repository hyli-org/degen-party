use std::sync::Arc;

use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use client_sdk::{rest_client::NodeApiHttpClient, transaction_builder::TxExecutorHandler};
use hyle_modules::{
    bus::SharedMessageBus, module_bus_client, module_handle_messages, modules::Module,
};
use sdk::{
    api::APIRegisterContract, utils::as_hyle_output, BlobTransaction, ContractName,
    StateCommitment, ZkContract,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[cfg(not(feature = "fake_proofs"))]
use {
    anyhow::bail,
    sha2::{Digest, Sha256},
    std::fs::File,
    std::io::Write,
};

/// Inbound transaction message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum InboundTxMessage {
    // TODO: do this in sync with blobs
    RegisterContract((ContractName, StateCommitment)),
    NewTransaction(BlobTransaction),
}

module_bus_client! {
#[derive(Debug)]
pub struct FakeLaneManagerBusClient {
    receiver(InboundTxMessage),
}
}

/// Fake Lane Manager module
pub struct FakeLaneManager {
    bus: FakeLaneManagerBusClient,
    hyle_client: Arc<NodeApiHttpClient>,
}

impl Module for FakeLaneManager {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, _ctx: Self::Context) -> Result<Self> {
        // Initialize Hylé client
        let hyle_client = Arc::new(NodeApiHttpClient::new("http://localhost:4321".to_string())?);

        Ok(Self {
            bus: FakeLaneManagerBusClient::new_from_bus(bus.new_handle()).await,
            hyle_client,
        })
    }

    async fn run(&mut self) -> Result<()> {
        info!("Fake Lane Manager is running");

        module_handle_messages! {
            on_bus self.bus,
            listen<InboundTxMessage> msg => {
                if let Err(e) = self.process_transaction(msg).await {
                    error!("Error processing transaction: {:?}", e);
                    break;
                }
            }
        };

        Ok(())
    }
}

impl FakeLaneManager {
    async fn process_transaction(&mut self, tx: InboundTxMessage) -> Result<()> {
        match tx {
            InboundTxMessage::RegisterContract((contract_name, state_commitment)) => {
                match self.hyle_client.get_contract(&contract_name).await {
                    Ok(_) => {
                        info!("✅ {} contract is up to date", contract_name);
                    }
                    Err(_) => {
                        info!("🚀 Registering {} contract", contract_name);
                        self.register_contract(contract_name, state_commitment)
                            .await?;
                    }
                }
            }
            InboundTxMessage::NewTransaction(tx) => {
                // Send the transaction to the Hylé node
                let tx_hash = self.hyle_client.send_tx_blob(&tx).await?;
                info!(
                    "Transaction successfully sent to the blockchain. Hash: {}",
                    tx_hash
                );
            }
        }
        Ok(())
    }

    async fn register_contract(
        &mut self,
        contract_name: ContractName,
        state_commitment: StateCommitment,
    ) -> Result<()> {
        #[cfg(not(feature = "fake_proofs"))]
        let vk = {
            // Load the VK from local file
            let vk_path = &format!("vk_and_hash_{}.bin", contract_name);
            let vk = if std::path::Path::new(vk_path).exists() {
                let vk_elf = std::fs::read(vk_path)?;
                let vk: Vec<u8> = vk_elf[0..vk_elf.len() - 32].to_vec();
                let elf_hash: Vec<u8> = vk_elf[vk_elf.len() - 32..].to_vec();
                // Verify the hash of the ELF file
                let mut hasher = Sha256::new();
                let elf = match contract_name.0.as_str() {
                    "board_game" => contracts::BOARD_GAME_ELF,
                    "crash_game" => contracts::CRASH_GAME_ELF,
                    _ => bail!("Unknown contract name: {}", contract_name),
                };
                hasher.update(elf);
                let computed_hash = hasher.finalize().to_vec();
                if computed_hash != elf_hash {
                    None
                } else {
                    Some(vk)
                }
            } else {
                None
            };

            let vk = match vk {
                Some(vk) => vk,
                None => {
                    let client = sp1_sdk::ProverClient::from_env();
                    let elf = match contract_name.0.as_str() {
                        "board_game" => contracts::BOARD_GAME_ELF,
                        "crash_game" => contracts::CRASH_GAME_ELF,
                        _ => bail!("Unknown contract name: {}", contract_name),
                    };
                    let (_, vk) = client.setup(elf);
                    let vk = serde_json::to_vec(&vk)?;
                    // Save it locally along with hash of elf
                    // Compute the hash of the ELF file
                    let mut hasher = Sha256::new();
                    hasher.update(elf);
                    let elf_hash = hasher.finalize();

                    // Save the vk and hash locally
                    let mut file = File::create(vk_path)?;
                    file.write_all(&vk)?;
                    file.write_all(&elf_hash)?;
                    vk
                }
            };
            vk
        };

        // Send the transaction to register the contract
        #[cfg(feature = "fake_proofs")]
        let register_tx = APIRegisterContract {
            verifier: "test".into(),
            program_id: sdk::ProgramId(vec![0, 1, 2, 3]),
            state_commitment,
            contract_name: contract_name.clone(),
            timeout_window: Some(100),
        };
        #[cfg(not(feature = "fake_proofs"))]
        let register_tx = APIRegisterContract {
            verifier: "sp1-4".into(),
            program_id: sdk::ProgramId(vk),
            state_commitment,
            contract_name: contract_name.clone(),
            timeout_window: Some(100),
        };
        let res = self.hyle_client.register_contract(&register_tx).await?;

        tracing::warn!(
            "✅ Register contract for {} tx sent. Tx hash: {}",
            contract_name,
            res
        );

        Ok(())
    }
}

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
        borsh::to_vec(&self.state).map_err(|e| e.to_string())
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
        borsh::to_vec(&self.state).map_err(|e| e.to_string())
    }
}
