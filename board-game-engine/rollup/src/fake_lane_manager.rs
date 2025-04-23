use std::sync::Arc;

use anyhow::{bail, Result};
use hyle::{
    bus::BusMessage, module_bus_client, module_handle_messages, rest::client::NodeApiHttpClient,
    utils::modules::Module,
};
use hyle_contract_sdk::{
    api::APIRegisterContract, BlobIndex, BlobTransaction, Calldata, ContractName, Hashed,
    ProofTransaction, StateCommitment,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[cfg(feature = "fake_proofs")]
use hyle_contract_sdk::{guest::execute, ProofData};
#[cfg(not(feature = "fake_proofs"))]
use {
    hyle_client_sdk::helpers::sp1::SP1Prover,
    sha2::{Digest, Sha256},
    sp1_sdk::ProverClient,
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
    NewProofRequest((Vec<u8>, BlobIndex, BlobTransaction)),
}

impl BusMessage for InboundTxMessage {}

module_bus_client! {
#[derive(Debug)]
pub struct FakeLaneManagerBusClient {
    receiver(InboundTxMessage),
}
}

/// The ELF file for the Succinct RISC-V zkVM.
#[cfg(not(clippy))]
mod elfs {
    pub const BOARD_GAME_ELF: &[u8] = sp1_sdk::include_elf!("board-game-engine");
    pub const CRASH_GAME_ELF: &[u8] = sp1_sdk::include_elf!("crash_game");
}
#[cfg(clippy)]
#[allow(dead_code, unused)]
mod elfs {
    pub const BOARD_GAME_ELF: &[u8] = &[0, 1, 2, 3];
    pub const CRASH_GAME_ELF: &[u8] = &[0, 1, 2, 3];
}
#[allow(dead_code, unused)]
use elfs::*;

/// Fake Lane Manager module
pub struct FakeLaneManager {
    bus: FakeLaneManagerBusClient,
    hyle_client: Arc<NodeApiHttpClient>,
}

impl Module for FakeLaneManager {
    type Context = Arc<crate::Context>;

    async fn build(ctx: Self::Context) -> Result<Self> {
        // Initialize Hylé client
        let hyle_client = Arc::new(NodeApiHttpClient::new("http://localhost:4321".to_string())?);

        Ok(Self {
            bus: FakeLaneManagerBusClient::new_from_bus(ctx.bus.new_handle()).await,
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
                self.register_contract(contract_name, state_commitment)
                    .await?;
            }
            InboundTxMessage::NewTransaction(tx) => {
                // Send the transaction to the Hylé node
                let tx_hash = self.hyle_client.send_tx_blob(&tx).await?;
                info!(
                    "Transaction successfully sent to the blockchain. Hash: {}",
                    tx_hash
                );
            }
            InboundTxMessage::NewProofRequest((state, index, tx)) => {
                self.generate_and_send_proof(state, index, tx).await?;
            }
        }
        Ok(())
    }

    async fn generate_and_send_proof(
        &self,
        state: Vec<u8>,
        index: BlobIndex,
        tx: BlobTransaction,
    ) -> Result<()> {
        let Some(contract_name) = tx.blobs.get(index.0).map(|b| b.contract_name.clone()) else {
            bail!("Blob index {} not found in transaction", index);
        };
        info!("Generating proof for contract {}", contract_name);
        let calldata = Calldata {
            tx_hash: tx.hashed(),
            identity: tx.identity.clone(),
            blobs: hyle_contract_sdk::IndexedBlobs(
                tx.blobs
                    .iter()
                    .enumerate()
                    .map(|(i, b)| (BlobIndex(i), b.clone()))
                    .collect(),
            ),
            tx_blob_count: tx.blobs.len(),
            index,
            tx_ctx: None,
            private_input: vec![],
        };

        let client = self.hyle_client.clone();
        #[cfg(feature = "fake_proofs")]
        {
            info!("Fake proving TX: {:?}", tx.hashed());

            let ho = match contract_name.0.as_str() {
                "board_game" => execute::<board_game_engine::game::GameState>(&state, &calldata),
                "crash_game" => execute::<crash_game::GameState>(&state, &calldata),
                _ => bail!("Unknown contract name: {}", contract_name),
            };
            let proof_tx = ProofTransaction {
                proof: ProofData(borsh::to_vec(&vec![ho]).unwrap()),
                contract_name: contract_name.clone(),
            };

            let proof_tx_hash = client.send_tx_proof(&proof_tx).await?;
            info!(
                "Fake Proof transaction sent for contract {}, tx {}. Hash: {}",
                contract_name,
                tx.hashed(),
                proof_tx_hash
            );
        }
        #[cfg(not(feature = "fake_proofs"))]
        tokio::spawn(async move {
            info!("Ready to prove TX: {:?}", tx.hashed());
            if let Err(e) = (async || {
                let prover = match contract_name.0.as_str() {
                    "board_game" => SP1Prover::new(BOARD_GAME_ELF),
                    "crash_game" => SP1Prover::new(CRASH_GAME_ELF),
                    _ => bail!("Unknown contract name: {}", contract_name),
                };
                let proof = prover.prove(state, calldata).await?;

                let proof_tx = ProofTransaction {
                    proof,
                    contract_name: contract_name.clone(),
                };

                let proof_tx_hash = client.send_tx_proof(&proof_tx).await?;
                info!(
                    "Proof transaction sent for contract {}, tx {}. Hash: {}",
                    contract_name,
                    tx.hashed(),
                    proof_tx_hash
                );
                Ok::<_, anyhow::Error>(())
            })()
            .await
            {
                error!("Error generating proof for {}: {}", tx.hashed(), e);
            }
        });
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
                    "board_game" => BOARD_GAME_ELF,
                    "crash_game" => CRASH_GAME_ELF,
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
                    let client = ProverClient::from_env();
                    let elf = match contract_name.0.as_str() {
                        "board_game" => BOARD_GAME_ELF,
                        "crash_game" => CRASH_GAME_ELF,
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
            program_id: hyle_contract_sdk::ProgramId(vec![0, 1, 2, 3]),
            state_commitment,
            contract_name: contract_name.clone(),
        };
        #[cfg(not(feature = "fake_proofs"))]
        let register_tx = APIRegisterContract {
            verifier: "sp1-4".into(),
            program_id: hyle_contract_sdk::ProgramId(vk),
            state_commitment,
            contract_name: contract_name.clone(),
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
