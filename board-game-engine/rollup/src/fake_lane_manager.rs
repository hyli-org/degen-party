use std::sync::Arc;

use anyhow::Result;
use hyle::{
    bus::BusMessage, module_bus_client, module_handle_messages, rest::client::NodeApiHttpClient,
    utils::modules::Module,
};
use hyle_contract_sdk::BlobTransaction;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// Inbound transaction message type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum InboundTxMessage {
    NewTransaction(BlobTransaction),
}

impl BusMessage for InboundTxMessage {}

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
                    error!("Error processing transaction: {}", e);
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
}
