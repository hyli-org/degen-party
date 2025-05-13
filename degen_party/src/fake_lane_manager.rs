use std::sync::Arc;

use anyhow::Result;
use client_sdk::rest_client::NodeApiHttpClient;
use hyle_modules::{
    bus::SharedMessageBus, module_bus_client, module_handle_messages, modules::Module,
};
use sdk::BlobTransaction;
use tracing::{error, info};

module_bus_client! {
#[derive(Debug)]
pub struct FakeLaneManagerBusClient {
    receiver(BlobTransaction),
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
            listen<BlobTransaction> msg => {
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
    async fn process_transaction(&mut self, tx: BlobTransaction) -> Result<()> {
        // Send the transaction to the Hylé node
        let tx_hash = self.hyle_client.send_tx_blob(&tx).await?;
        info!(
            "Transaction successfully sent to the blockchain. Hash: {}",
            tx_hash
        );
        Ok(())
    }
}
