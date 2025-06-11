use ::crash_game::ChainEvent;
use anyhow::Result;
use board_game::game::GameEvent;
use borsh::{BorshDeserialize, BorshSerialize};
use client_sdk::transaction_builder::TxExecutorHandler;
use crash_game::CrashGameEvent;
use game_state::GameStateEvent;
use hyle_modules::{
    bus::{BusClientSender, SharedMessageBus},
    log_error, module_bus_client, module_handle_messages,
    modules::{
        websocket::{WsBroadcastMessage, WsInMessage},
        Module, ModulesHandler,
    },
};
use sdk::{
    hyle_model_utils::TimestampMs, BlobTransaction, Calldata, ContractName, Hashed, HyleOutput,
    Identity, LaneId, MempoolStatusEvent, NodeStateEvent, TransactionData, TxContext, TxHash, TxId,
};
use smt_token::client::tx_executor_handler::SmtTokenProvableState;
use std::fmt;
use std::{
    any::TypeId,
    time::{SystemTime, UNIX_EPOCH},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    path::PathBuf,
    vec,
};
use tokio::time::{self, Instant};
use wallet::client::tx_executor_handler::Wallet;

use crate::{
    fake_lane_manager::ConfirmedBlobTransaction,
    proving::{BoardGameExecutor, CrashGameExecutor},
    AuthenticatedMessage, Context, CryptoContext, InboundWebsocketMessage,
    OutboundWebsocketMessage,
};

pub mod crash_game;
pub mod game_state;

pub struct RollupExecutor {
    bus: RollupExecutorBusClient,
    data_directory: PathBuf,
    crypto: Arc<CryptoContext>,
    store: RollupExecutorStore,
    // Convenience, TODO refactor this ?
    last_claim_reward: Instant,
}

impl Deref for RollupExecutor {
    type Target = RollupExecutorStore;
    fn deref(&self) -> &Self::Target {
        &self.store
    }
}
impl DerefMut for RollupExecutor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

pub trait RollupContract: TxExecutorHandler + Debug + Send + Sync {
    fn clone_box(&self) -> Box<dyn RollupContract>;
    fn borsh_serialize_box(&self) -> Result<Vec<u8>, std::io::Error>;
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T> RollupContract for T
where
    T: 'static
        + TxExecutorHandler
        + BorshSerialize
        + BorshDeserialize
        + Clone
        + Debug
        + Send
        + Sync,
{
    fn clone_box(&self) -> Box<dyn RollupContract> {
        Box::new(self.clone())
    }
    fn borsh_serialize_box(&self) -> Result<Vec<u8>, std::io::Error> {
        borsh::to_vec(self)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// Wrapper for contract trait objects with manual Clone/Debug
pub struct ContractBox {
    type_id: TypeId,
    inner: Box<dyn RollupContract + Send + Sync>,
}

impl ContractBox {
    pub fn new<T>(inner: T) -> Self
    where
        T: TxExecutorHandler
            + Clone
            + Debug
            + BorshSerialize
            + BorshDeserialize
            + Send
            + Sync
            + 'static,
    {
        let type_id = TypeId::of::<T>();
        Self {
            type_id,
            inner: Box::new(inner),
        }
    }
}

impl std::ops::Deref for ContractBox {
    type Target = dyn RollupContract;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
impl std::ops::DerefMut for ContractBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl Clone for ContractBox {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            inner: self.inner.clone_box(),
        }
    }
}

impl Debug for ContractBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContractBox {{ {:?} }}", self.inner)
    }
}

impl BorshSerialize for ContractBox {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Serialize to a vector of bytes so we can deserialize it as one.
        borsh::to_writer(writer, &self.inner.borsh_serialize_box()?)
    }
}

enum DataQuality {
    Internal,
    Mempool,
    Consensus,
}

#[derive(Clone, BorshSerialize)]
pub struct RollupExecutorStore {
    unsettled_txs: Vec<(BlobTransaction, TxContext)>,
    pub contracts: HashMap<ContractName, ContractBox>,
    pub settled_state: HashMap<ContractName, ContractBox>,
    board_game: ContractName,
    crash_game: ContractName,
}

#[derive(Default, BorshDeserialize)]
pub struct DeserRollupExecutorStore {
    unsettled_txs: Vec<(BlobTransaction, TxContext)>,
    contracts: HashMap<ContractName, Vec<u8>>,
    settled_state: HashMap<ContractName, Vec<u8>>,
    board_game: ContractName,
    crash_game: ContractName,
}

pub struct RollupExecutorCtx {
    pub common: Arc<Context>,
    pub initial_contracts: HashMap<ContractName, ContractBox>,
    pub contract_deserializer: Box<dyn Fn(Vec<u8>, &ContractName) -> ContractBox + Send + Sync>,
}

module_bus_client! {
#[derive(Debug)]
pub struct RollupExecutorBusClient {
    sender(BlobTransaction),
    sender(WsBroadcastMessage<OutboundWebsocketMessage>),
    receiver(WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>),
    receiver(NodeStateEvent),
    receiver(MempoolStatusEvent),
    receiver(ConfirmedBlobTransaction),
}
}
impl Module for RollupExecutor {
    type Context = RollupExecutorCtx;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        let bus = RollupExecutorBusClient::new_from_bus(bus.new_handle()).await;

        let data_directory = ctx.common.data_directory.clone();
        let file = data_directory.join("rollup_executor.bin");

        let store = match Self::load_from_disk::<DeserRollupExecutorStore>(file.as_path()) {
            Some(store) => RollupExecutorStore::deser_with(store, ctx.contract_deserializer),
            None => RollupExecutorStore {
                contracts: ctx.initial_contracts.clone(),
                settled_state: ctx.initial_contracts,
                unsettled_txs: Vec::new(),
                board_game: ctx.common.board_game.clone(),
                crash_game: ctx.common.crash_game.clone(),
            },
        };

        Ok(RollupExecutor {
            bus,
            store,
            data_directory,
            crypto: ctx.common.crypto.clone(),
            last_claim_reward: Instant::now(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        let mut update_interval = time::interval(std::time::Duration::from_millis(50));

        module_handle_messages! {
            on_bus self.bus,
            listen<WsInMessage<AuthenticatedMessage<InboundWebsocketMessage>>> msg => {
                let AuthenticatedMessage {
                    message,
                    identity,
                    uuid,
                    identity_blobs
                } = msg.message;
                if let InboundWebsocketMessage::GameState(event) = message {
                    if let Err(e) = self.handle_user_message(event, identity, &uuid, identity_blobs).await {
                        tracing::warn!("Error handling event: {:?}", e);
                    }
                 } else if let InboundWebsocketMessage::CrashGame(event) = message {
                    if let Err(e) = self.handle_player_message(event, identity, &uuid, identity_blobs).await {
                        tracing::warn!("Error handling player message: {:?}", e);
                    }
                }
            }
            listen<NodeStateEvent> event => {
                _ = log_error!(self.handle_node_state_event(event).await, "handle note state event")
            }
            listen<MempoolStatusEvent> event => {
                // Temporarily off.
                //_ = log_error!(self.handle_mempool_status_event(event).await, "handle mempool status event")
            }
            listen<ConfirmedBlobTransaction> event => {
                _ = log_error!(self.handle_optimistic_tx(event.0, event.1, None, DataQuality::Internal).await, "handle optimistic tx");
            }
            _ = update_interval.tick() => {
                _ = log_error!(self.board_game_on_tick().await, "board game on tick");
                _ = log_error!(self.crash_game_on_tick().await, "crash game on tick");
            }
        };

        let _ = log_error!(
            Self::save_on_disk::<RollupExecutorStore>(
                self.data_directory
                    // TODO: Multi-contract: use a canonical file name or one per contract
                    .join("rollup_executor.bin")
                    .as_path(),
                &self.store,
            ),
            "Saving prover"
        );

        Ok(())
    }
}

impl RollupExecutor {
    async fn handle_node_state_event(&mut self, event: NodeStateEvent) -> Result<()> {
        match event {
            NodeStateEvent::NewBlock(block) => {
                if !block.txs.is_empty()
                    || !block.timed_out_txs.is_empty()
                    || !block.failed_txs.is_empty()
                {
                    tracing::info!("Handling new block {}", block.block_height);
                }
                if let Some(eff) = block.registered_contracts.get(&ContractName::new("wallet")) {
                    let wallet = Wallet::new(&Some(
                        borsh::from_slice(
                            eff.2
                                .as_ref()
                                .expect("Wallet contract should have metadata"),
                        )
                        .expect("Failed to deserialize wallet metadata"),
                    ))
                    .expect("Failed to create wallet");
                    self.contracts.insert(
                        ContractName::new("wallet"),
                        ContractBox::new(wallet.clone()),
                    );
                    self.settled_state
                        .insert(ContractName::new("wallet"), ContractBox::new(wallet));
                }

                for (TxId(_, tx_hash), tx) in block.txs.iter() {
                    if let TransactionData::Blob(blob_tx) = &tx.transaction_data {
                        if let Err(e) = self
                            .handle_optimistic_tx(
                                block.lane_ids.get(tx_hash).cloned().unwrap_or_default(),
                                blob_tx.clone(),
                                block.build_tx_ctx(tx_hash).ok(),
                                DataQuality::Consensus,
                            )
                            .await
                        {
                            tracing::info!("Error handling optimistic tx: {:?}", e);
                        }
                    }
                }
                self.handle_successful_transactions(block.successful_txs);
                let merged_set: HashSet<_> = block
                    .timed_out_txs
                    .iter()
                    .chain(block.failed_txs.iter())
                    .cloned()
                    .collect();
                self.cancel_tx(merged_set)?;
                Ok(())
            }
        }
    }
    async fn handle_mempool_status_event(&mut self, event: MempoolStatusEvent) -> Result<()> {
        if let MempoolStatusEvent::WaitingDissemination { tx, .. } = event {
            if let TransactionData::Blob(blob_tx) = tx.transaction_data {
                if let Err(e) = self
                    .handle_optimistic_tx(LaneId::default(), blob_tx, None, DataQuality::Mempool)
                    .await
                {
                    tracing::info!("Error handling optimistic tx in mempool: {:?}", e);
                }
            }
        }
        Ok(())
    }

    async fn handle_optimistic_tx(
        &mut self,
        lane_id: LaneId,
        blob_tx: BlobTransaction,
        tx_ctx: Option<TxContext>,
        quality: DataQuality,
    ) -> Result<()> {
        if let Some((tx, ctx)) = self
            .unsettled_txs
            .iter_mut()
            .find(|(tx, _)| tx.hashed() == blob_tx.hashed())
        {
            if matches!(quality, DataQuality::Consensus) {
                *tx = blob_tx;
                if let Some(tx_ctx) = tx_ctx {
                    tracing::info!(
                        "Transaction {} is already in the unsettled transactions, updating context",
                        tx.hashed()
                    );
                    tracing::debug!("Updating context: {:?} -> {:?}", ctx, tx_ctx);
                    *ctx = tx_ctx;
                }
                self.rerun_from_settled();
                return Ok(());
            }
            tracing::info!(
                "Transaction {} is already in the unsettled transactions",
                blob_tx.hashed()
            );
            return Ok(());
        }

        let tx_ctx = tx_ctx.unwrap_or(TxContext {
            lane_id,
            timestamp: TimestampMs(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()),
            ..Default::default()
        });

        let hyle_outputs = RollupExecutorStore::execute_blob_tx(
            &mut self.contracts,
            &blob_tx,
            Some(tx_ctx.clone()),
            false,
        );

        // If we have a success and the outputs are empty, then we ignored.
        if hyle_outputs.as_ref().map(|x| x.is_empty()).unwrap_or(false) {
            return Ok(());
        }

        // Always insert it in the list of unsettled TXs, even if we fail to execute it, as it might be a valid TX
        // depending on the fact that some future TX will e.g. timeout.
        self.unsettled_txs.push((blob_tx.clone(), tx_ctx));

        let hyle_outputs = hyle_outputs?;

        // Special for degen-party: process events and send updates to WS
        for (hyle_output, contract_name) in &hyle_outputs {
            if contract_name == &self.board_game {
                let events: Vec<GameEvent> =
                    borsh::from_slice(&hyle_output.program_outputs).unwrap();
                self.bus.send(WsBroadcastMessage {
                    message: OutboundWebsocketMessage::GameStateEvent(
                        GameStateEvent::StateUpdated {
                            state: Some(self.get_board_game().clone()),
                            events,
                            board_game: self.board_game.clone(),
                            crash_game: self.crash_game.clone(),
                        },
                    ),
                })?;
            } else if contract_name == &self.crash_game {
                let events: Vec<ChainEvent> =
                    borsh::from_slice(&hyle_output.program_outputs).unwrap();
                let state = Some(self.get_crash_game().clone());
                self.bus.send(WsBroadcastMessage {
                    message: OutboundWebsocketMessage::CrashGame(CrashGameEvent::StateUpdated {
                        state,
                        events,
                    }),
                })?;
            }
        }

        tracing::info!("Optimistically executed transaction {}", blob_tx.hashed(),);

        Ok(())
    }
}

impl RollupExecutorStore {
    fn deser_with(
        deser_store: DeserRollupExecutorStore,
        contract_deserializer: impl Fn(Vec<u8>, &ContractName) -> ContractBox,
    ) -> Self {
        let contracts = deser_store
            .contracts
            .into_iter()
            .map(|(name, data)| {
                let c = contract_deserializer(data, &name);
                (name, c)
            })
            .collect();
        let settled_state = deser_store
            .settled_state
            .into_iter()
            .map(|(name, data)| {
                let c = contract_deserializer(data, &name);
                (name, c)
            })
            .collect();
        Self {
            unsettled_txs: deser_store.unsettled_txs,
            contracts,
            settled_state,
            board_game: deser_store.board_game,
            crash_game: deser_store.crash_game,
        }
    }

    /// This function executes the blob transaction and returns the outputs of the contract.
    /// Errors on unknown blobs (if we care about the TX at all) or unsuccessful outputs.
    pub fn execute_blob_tx(
        contracts: &mut HashMap<ContractName, ContractBox>,
        blob_tx: &BlobTransaction,
        tx_ctx: Option<TxContext>,
        process_partial: bool,
    ) -> anyhow::Result<Vec<(HyleOutput, ContractName)>> {
        // 1. Clone all involved contracts' state
        let mut temp_contracts: BTreeMap<ContractName, ContractBox> = BTreeMap::new();
        let mut skipped_contracts = 0;
        for blob in &blob_tx.blobs {
            if let Some(contract) = contracts.get(&blob.contract_name) {
                temp_contracts.insert(blob.contract_name.clone(), contract.clone());
            // Ignore check secret - we can't verify it but we'll assume it's OK for now.
            } else if &blob.contract_name.0 != "check_secret" {
                skipped_contracts += 1;
            }
        }
        if temp_contracts.is_empty() {
            // we don't care about this TX, ignore.
            return Ok(vec![]);
        }
        if skipped_contracts > 0 {
            if process_partial {
                tracing::debug!(
                    "Processing partial blob transaction {} with {} skipped contracts",
                    blob_tx.hashed(),
                    skipped_contracts
                );
            } else {
                anyhow::bail!(
                    "Tried to execute blob transaction {} but we cannot handle some blobs, {} skipped",
                    blob_tx.hashed(),
                    skipped_contracts
                );
            }
        }
        let mut hyle_outputs = vec![];
        // 2. Execute all blobs, mutating the correct contract in the map
        for (blob_index, blob) in blob_tx.blobs.iter().enumerate() {
            let Some(contract) = temp_contracts.get_mut(&blob.contract_name) else {
                continue;
            };

            let calldata = Calldata {
                identity: blob_tx.identity.clone(),
                tx_hash: blob_tx.hashed(),
                private_input: vec![],
                blobs: blob_tx.blobs.clone().into(),
                index: blob_index.into(),
                tx_ctx: tx_ctx.clone(),
                tx_blob_count: blob_tx.blobs.len(),
            };
            match contract.handle(&calldata) {
                Err(e) => {
                    anyhow::bail!(
                        "Error while executing tx {} on blob index {} for {}: {e}",
                        blob_tx.hashed(),
                        calldata.index,
                        blob.contract_name
                    );
                }
                Ok(hyle_output) => {
                    if !hyle_output.success {
                        anyhow::bail!(
                            "Hyle output for tx {} on blob index {} for {} is not successful: {:?}",
                            blob_tx.hashed(),
                            calldata.index,
                            blob.contract_name,
                            String::from_utf8(hyle_output.program_outputs.clone())
                                .unwrap_or(hex::encode(&hyle_output.program_outputs)),
                        );
                    }
                    hyle_outputs.push((hyle_output, blob.contract_name.clone()));
                }
            }
        }
        // 3. Blobs execution went fine. Update actual contracts.
        for (contract_name, contract) in temp_contracts {
            contracts.insert(contract_name, contract);
        }
        Ok(hyle_outputs)
    }

    pub fn rerun_from_settled(&mut self) {
        // Revert each contract to the settled state.
        for (contract_name, state) in &self.settled_state {
            self.contracts.insert(contract_name.clone(), state.clone());
        }
        // Re-execute all unsettled transactions from that safe state - ignore errors.
        for (blob_tx, tx_ctx) in &self.unsettled_txs {
            let _ =
                Self::execute_blob_tx(&mut self.contracts, blob_tx, Some(tx_ctx.clone()), false);
        }
    }

    /// This function is called when the transaction is confirmed as failed.
    /// It reverts the state and reexecutes all unsettled transaction after this one.
    pub fn cancel_tx(&mut self, tx_hashes: HashSet<TxHash>) -> anyhow::Result<()> {
        let mut removed = 0;
        for tx_hash in tx_hashes {
            let Some(tx_pos) = self
                .unsettled_txs
                .iter()
                .position(|(blob_tx, _)| blob_tx.hashed() == tx_hash)
            else {
                return Ok(());
            };
            tracing::warn!("Cancelling transaction {} at position {}", tx_hash, tx_pos);
            let _ = self.unsettled_txs.remove(tx_pos);
            removed += 1;
        }
        if removed > 0 {
            self.rerun_from_settled();
        }
        Ok(())
    }

    fn handle_successful_transactions(&mut self, successful_txs: Vec<TxHash>) {
        let mut successful = 0;
        for tx_hash in successful_txs {
            // Remove the transaction from unsettled transactions
            if let Some(pos) = self
                .unsettled_txs
                .iter()
                .position(|(tx, _)| tx.hashed() == tx_hash)
            {
                let (blob_tx, tx_ctx) = self.unsettled_txs.remove(pos);
                tracing::debug!(
                    "Transaction {} is successful, removing from unsettled",
                    tx_hash
                );
                successful += 1;
                if let Err(e) =
                    Self::execute_blob_tx(&mut self.settled_state, &blob_tx, Some(tx_ctx), true)
                {
                    // This _really_ should not happen, as we are executing a successful transaction on settled state.
                    // Probably indicates misconfiguration or desync from the chain.
                    tracing::error!(
                        "Error while executing settled transaction {}: {:?}",
                        tx_hash,
                        e
                    );
                }
            }
        }
        if successful > 0 {
            self.rerun_from_settled();
        }
    }
}

impl RollupExecutorStore {
    pub fn new(
        contracts: &[(ContractName, ContractBox)],
        board_game: ContractName,
        crash_game: ContractName,
    ) -> Self {
        Self {
            unsettled_txs: Vec::new(),
            contracts: contracts
                .iter()
                .map(|(name, contract)| (name.clone(), contract.clone()))
                .collect(),
            settled_state: contracts
                .iter()
                .map(|(name, contract)| (name.clone(), contract.clone()))
                .collect(),
            board_game,
            crash_game,
        }
    }
}

pub async fn setup_rollup_execution(
    ctx: Arc<crate::Context>,
    handler: &mut ModulesHandler,
) -> Result<()> {
    let board_game_executor = BoardGameExecutor {
        state: board_game::game::GameState::new(Identity::new(format!(
            "{}@secp256k1",
            ctx.crypto.public_key
        ))),
    };
    let crash_game_state: ::crash_game::GameState = ::crash_game::GameState::new(
        ctx.board_game.clone(),
        Identity::new(format!("{}@secp256k1", ctx.crypto.public_key,)),
    );
    let crash_game_executor = CrashGameExecutor {
        state: crash_game_state,
    };
    let board_game = ctx.board_game.clone();
    let crash_game = ctx.crash_game.clone();
    handler
        .build_module::<RollupExecutor>(RollupExecutorCtx {
            common: ctx.clone(),
            initial_contracts: [
                (
                    ctx.board_game.clone(),
                    ContractBox::new(board_game_executor.clone()),
                ),
                (
                    ctx.crash_game.clone(),
                    ContractBox::new(crash_game_executor.clone()),
                ),
                (
                    ContractName::new("oxygen"),
                    ContractBox::new(SmtTokenProvableState::default()),
                ),
                (
                    ContractName::new("oranj"),
                    ContractBox::new(SmtTokenProvableState::default()),
                ),
                (
                    ContractName::new("wallet"),
                    ContractBox::new(Wallet::new(&None).expect("Failed to create wallet")),
                ),
                (
                    ContractName::new("secp256k1"),
                    ContractBox::new(
                        hyle_modules::utils::native_verifier_handler::NativeVerifierHandler,
                    ),
                ),
            ]
            .into_iter()
            .collect::<HashMap<_, _>>(),
            contract_deserializer: Box::new(move |data, contract_name| {
                if contract_name == &board_game {
                    ContractBox::new(
                        borsh::from_slice::<BoardGameExecutor>(&data).expect("Bad serialized data"),
                    )
                } else if contract_name == &crash_game {
                    ContractBox::new(
                        borsh::from_slice::<CrashGameExecutor>(&data).expect("Bad serialized data"),
                    )
                } else if contract_name == &ContractName::new("oranj")
                    || contract_name == &ContractName::new("oxygen")
                {
                    ContractBox::new(
                        borsh::from_slice::<SmtTokenProvableState>(&data)
                            .expect("Bad serialized data"),
                    )
                } else if contract_name == &ContractName::new("wallet") {
                    ContractBox::new(
                        borsh::from_slice::<Wallet>(&data).expect("Bad serialized data"),
                    )
                } else if contract_name == &ContractName::new("secp256k1") {
                    ContractBox::new(
                        hyle_modules::utils::native_verifier_handler::NativeVerifierHandler,
                    )
                } else {
                    panic!("Unknown contract name: {}", contract_name);
                }
            }),
        })
        .await?;

    Ok(())
}
