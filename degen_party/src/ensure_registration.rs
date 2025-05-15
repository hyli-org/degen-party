use std::sync::Arc;

use anyhow::Result;
use client_sdk::rest_client::NodeApiHttpClient;
use hyle_modules::{
    bus::SharedMessageBus, module_bus_client, module_handle_messages, modules::Module,
};
use sdk::{api::APIRegisterContract, ContractName, Identity, StateCommitment, TxHash, ZkContract};

#[cfg(not(feature = "fake_proofs"))]
use {
    anyhow::bail,
    sha2::{Digest, Sha256},
    std::fs::File,
    std::io::Write,
};

module_bus_client! {
pub struct EnsureRegistrationBusClient {
}
}

pub struct EnsureRegistration {
    bus: EnsureRegistrationBusClient,
    hyle_client: Arc<NodeApiHttpClient>,
    board_game: ContractName,
    crash_game: ContractName,
}

impl Module for EnsureRegistration {
    type Context = Arc<crate::Context>;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        // Initialize Hylé client
        let hyle_client = ctx.client.clone();

        let mut module = Self {
            bus: EnsureRegistrationBusClient::new_from_bus(bus.new_handle()).await,
            hyle_client,
            board_game: ctx.board_game.clone(),
            crash_game: ctx.crash_game.clone(),
        };

        let a = ctx.client.get_contract(&ctx.board_game).await;
        let b = ctx.client.get_contract(&ctx.crash_game).await;

        if let (Ok(_), Ok(_)) = (a, b) {
            tracing::info!("Contracts already registered");
            return Ok(module);
        }

        module
            .register_contract(
                ctx.board_game.clone(),
                board_game::game::GameState::default().commit(),
            )
            .await?;
        module
            .register_contract(
                ctx.crash_game.clone(),
                crash_game::GameState::new(
                    ctx.board_game.clone(),
                    Identity::new(format!("{}@secp256k1", ctx.crypto.public_key,)),
                )
                .commit(),
            )
            .await?;

        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let a = ctx.client.get_contract(&ctx.board_game).await;
                let b = ctx.client.get_contract(&ctx.crash_game).await;
                if let (Ok(_), Ok(_)) = (a, b) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timeout waiting for contracts to be registered"))?;

        Ok(module)
    }

    async fn run(&mut self) -> Result<()> {
        module_handle_messages! {
            on_bus self.bus,
        };

        Ok(())
    }
}

impl EnsureRegistration {
    async fn register_contract(
        &mut self,
        contract_name: ContractName,
        state_commitment: StateCommitment,
    ) -> Result<TxHash> {
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
                let elf = {
                    if contract_name == self.board_game {
                        contracts::BOARD_GAME_ELF
                    } else if contract_name == self.crash_game {
                        contracts::CRASH_GAME_ELF
                    } else {
                        bail!("Unknown contract name: {}", contract_name)
                    }
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

            match vk {
                Some(vk) => vk,
                None => {
                    let client = sp1_sdk::ProverClient::from_env();
                    let elf = {
                        if contract_name == self.board_game {
                            contracts::BOARD_GAME_ELF
                        } else if contract_name == self.crash_game {
                            contracts::CRASH_GAME_ELF
                        } else {
                            bail!("Unknown contract name: {}", contract_name)
                        }
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
            }
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

        Ok(res)
    }
}
