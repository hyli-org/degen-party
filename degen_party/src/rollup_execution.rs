use anyhow::Result;
use borsh::{BorshDeserialize, BorshSerialize};
use client_sdk::transaction_builder::TxExecutorHandler;
use hyle_modules::{
    bus::{BusClientSender, SharedMessageBus},
    log_error, module_bus_client, module_handle_messages,
    modules::Module,
};
use sdk::{
    BlobTransaction, Calldata, ContractName, Hashed, HyleOutput, LaneId, MempoolStatusEvent,
    NodeStateEvent, TransactionData, TxContext, TxHash, ValidatorPublicKey,
};
use std::any::TypeId;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
    path::PathBuf,
    vec,
};

pub struct RollupExecutor {
    bus: RollupExecutorBusClient,
    data_directory: PathBuf,
    store: RollupExecutorStore,
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

#[derive(BorshSerialize)]
pub struct RollupExecutorStore {
    validator_lane_id: ValidatorPublicKey,
    unsettled_txs: Vec<(BlobTransaction, TxContext)>,
    contracts: HashMap<ContractName, ContractBox>,
    state_history: HashMap<ContractName, Vec<(TxHash, ContractBox)>>,
}

#[derive(Default, BorshDeserialize)]
pub struct DeserRollupExecutorStore {
    validator_lane_id: ValidatorPublicKey,
    unsettled_txs: Vec<(BlobTransaction, TxContext)>,
    contracts: HashMap<ContractName, Vec<u8>>,
    state_history: HashMap<ContractName, Vec<(TxHash, Vec<u8>)>>,
}

pub struct RollupExecutorCtx {
    pub data_directory: PathBuf,
    pub contract_name: ContractName,
    pub validator_lane_id: ValidatorPublicKey,
    pub contract_deserializer: fn(Vec<u8>, &ContractName) -> ContractBox,
}

#[derive(Debug, Clone)]
pub enum RollupExecutorEvent {
    /// Event sent when a blob is executed as successfully
    #[allow(dead_code)]
    TxExecutionSuccess(
        BlobTransaction,
        Vec<(ContractName, ContractBox)>,
        Vec<HyleOutput>,
    ),
    /// Event sent when a blob is reverted
    /// After a revert, the contract state is recalculated
    #[allow(dead_code)]
    RevertedTx(BlobTransaction, Vec<(ContractName, ContractBox)>),
}

module_bus_client! {
#[derive(Debug)]
pub struct RollupExecutorBusClient {
    sender(RollupExecutorEvent),
    receiver(NodeStateEvent),
    receiver(MempoolStatusEvent),
}
}
impl Module for RollupExecutor {
    type Context = RollupExecutorCtx;

    async fn build(bus: SharedMessageBus, ctx: Self::Context) -> Result<Self> {
        let bus = RollupExecutorBusClient::new_from_bus(bus.new_handle()).await;

        let data_directory = ctx.data_directory.clone();
        let file =
            data_directory.join(format!("rollup_executor_{}.bin", ctx.contract_name).as_str());

        let store = match Self::load_from_disk::<DeserRollupExecutorStore>(file.as_path()) {
            Some(store) => RollupExecutorStore::deser_with(store, ctx.contract_deserializer),
            None => RollupExecutorStore {
                contracts: HashMap::new(),
                state_history: HashMap::new(),
                unsettled_txs: Vec::new(),
                validator_lane_id: ctx.validator_lane_id.clone(),
            },
        };

        Ok(RollupExecutor {
            bus,
            store,
            data_directory,
        })
    }

    async fn run(&mut self) -> Result<()> {
        module_handle_messages! {
            on_bus self.bus,
            listen<NodeStateEvent> event => {
                _ = log_error!(self.handle_node_state_event(event).await, "handle note state event")
            }

            listen<MempoolStatusEvent> event => {
                _ = log_error!(self.handle_mempool_status_event(event).await, "handle mempool status event");
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
                self.handle_successful_transactions(block.successful_txs);
                let merged_set: HashSet<_> = block
                    .timed_out_txs
                    .iter()
                    .chain(block.failed_txs.iter())
                    .cloned()
                    .collect();
                for tx_hash in merged_set.iter() {
                    let blob_tx = self.cancel_tx(tx_hash)?;
                    // For all contracts in the tx, send their state
                    let mut contract_states = vec![];
                    for blob in &blob_tx.blobs {
                        if let Some(contract) = self.contracts.get(&blob.contract_name) {
                            // TODO: This will panic if clone_typed is required. For now, use clone if available.
                            contract_states.push((blob.contract_name.clone(), contract.clone()));
                        }
                    }
                    self.bus
                        .send(RollupExecutorEvent::RevertedTx(blob_tx, contract_states))?;
                }
                Ok(())
            }
        }
    }

    async fn handle_mempool_status_event(&mut self, event: MempoolStatusEvent) -> Result<()> {
        match event {
            MempoolStatusEvent::WaitingDissemination { tx, .. } => {
                let tx_ctx = Some(TxContext {
                    lane_id: LaneId(self.validator_lane_id.clone()),
                    ..Default::default()
                });
                if let TransactionData::Blob(blob_tx) = tx.transaction_data {
                    let hyle_outputs = self.execute_blob_tx(&blob_tx, tx_ctx)?;
                    // For all contracts in the tx, send their state
                    let mut contract_states = vec![];
                    for blob in &blob_tx.blobs {
                        if let Some(contract) = self.contracts.get(&blob.contract_name) {
                            // TODO: This will panic if clone_typed is required. For now, use clone if available.
                            contract_states.push((blob.contract_name.clone(), contract.clone()));
                        }
                    }
                    self.bus.send(RollupExecutorEvent::TxExecutionSuccess(
                        blob_tx,
                        contract_states,
                        hyle_outputs,
                    ))?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl RollupExecutorStore {
    fn deser_with(
        deser_store: DeserRollupExecutorStore,
        contract_deserializer: fn(Vec<u8>, &ContractName) -> ContractBox,
    ) -> Self {
        let contracts = deser_store
            .contracts
            .into_iter()
            .map(|(name, data)| {
                let c = contract_deserializer(data, &name);
                (name, c)
            })
            .collect();
        let state_history = deser_store
            .state_history
            .into_iter()
            .map(|(name, history)| {
                let h = history
                    .into_iter()
                    .map(|(tx_hash, state)| (tx_hash, contract_deserializer(state, &name)))
                    .collect();
                (name, h)
            })
            .collect();
        Self {
            validator_lane_id: deser_store.validator_lane_id,
            unsettled_txs: deser_store.unsettled_txs,
            contracts,
            state_history,
        }
    }

    /// This function executes the blob transaction and returns the outputs of the contract.
    /// It also keeps track of the transaction as unsettled and the state history.
    pub fn execute_blob_tx(
        &mut self,
        blob_tx: &BlobTransaction,
        tx_ctx: Option<TxContext>,
    ) -> anyhow::Result<Vec<HyleOutput>> {
        // 1. Snapshot all involved contracts' state
        let mut contract_snapshots: BTreeMap<ContractName, ContractBox> = BTreeMap::new();
        for blob in &blob_tx.blobs {
            if let Some(contract) = self.contracts.get(&blob.contract_name) {
                contract_snapshots.insert(blob.contract_name.clone(), contract.clone());
            }
        }
        let mut hyle_outputs = vec![];
        let mut affected_contracts = vec![];
        // 2. Execute all blobs, mutating the correct contract in the map
        for (blob_index, blob) in blob_tx.blobs.iter().enumerate() {
            let contract = self
                .contracts
                .entry(blob.contract_name.clone())
                .or_insert_with(|| {
                    // If contract doesn't exist, use a default (should be provided elsewhere)
                    // For now, panic to catch missing default
                    panic!(
                        "Missing default state for contract: {:?}",
                        blob.contract_name
                    )
                });
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
                    // Revert all affected contracts to their snapshot
                    for (name, snapshot) in contract_snapshots.iter() {
                        self.contracts.insert(name.clone(), snapshot.clone());
                    }
                    anyhow::bail!(
                        "Error while executing tx {} on blob index {} for {}: {e}",
                        blob_tx.hashed(),
                        calldata.index,
                        blob.contract_name
                    );
                }
                Ok(hyle_output) => {
                    hyle_outputs.push(hyle_output);
                    if !affected_contracts.contains(&blob.contract_name) {
                        affected_contracts.push(blob.contract_name.clone());
                    }
                }
            }
        }
        // 3. Blobs execution went fine. Track as unsettled
        self.unsettled_txs
            .push((blob_tx.clone(), tx_ctx.clone().unwrap()));
        // 4. Update state history for all affected contracts
        for contract_name in affected_contracts {
            let contract = self.contracts.get(&contract_name).unwrap().clone();
            self.state_history
                .entry(contract_name)
                .or_default()
                .push((blob_tx.hashed(), contract));
        }
        Ok(hyle_outputs)
    }

    /// This function is called when the transaction is confirmed as failed.
    /// It reverts the state and reexecutes all unsettled transaction after this one.
    pub fn cancel_tx(&mut self, tx_hash: &TxHash) -> anyhow::Result<BlobTransaction> {
        let tx_pos = self
            .unsettled_txs
            .iter()
            .position(|(blob_tx, _)| blob_tx.hashed() == *tx_hash)
            .ok_or(anyhow::anyhow!(
                "Transaction not found in unsettled transactions"
            ))?;
        let (poped_tx, _) = self.unsettled_txs.remove(tx_pos);
        // 1. Find all contracts affected by this tx
        let mut affected_contracts = vec![];
        for blob in &poped_tx.blobs {
            if self.contracts.contains_key(&blob.contract_name) {
                affected_contracts.push(blob.contract_name.clone());
            }
        }
        // 2. Revert each contract to the state before this tx
        for contract_name in &affected_contracts {
            if let Some(history) = self.state_history.get_mut(contract_name) {
                if let Some((_, state)) = history.get(tx_pos) {
                    self.contracts.insert(contract_name.clone(), state.clone());
                    history.truncate(tx_pos);
                } else {
                    anyhow::bail!("State history not found for the cancelled transaction");
                }
            }
        }
        // 3. Re-execute all unsettled transactions after the cancelled one
        let reexecute_txs: Vec<(BlobTransaction, TxContext)> =
            self.unsettled_txs.drain(tx_pos..).collect();
        for (blob_tx, tx_ctx) in reexecute_txs.iter() {
            let _ = self.execute_blob_tx(blob_tx, Some(tx_ctx.clone()))?;
        }
        Ok(poped_tx)
    }

    fn handle_successful_transactions(&mut self, successful_txs: Vec<TxHash>) {
        for tx_hash in successful_txs {
            // Remove the transaction from unsettled transactions
            if let Some(pos) = self
                .unsettled_txs
                .iter()
                .position(|(tx, _)| tx.hashed() == tx_hash)
            {
                let (blob_tx, _) = self.unsettled_txs.remove(pos);
                // For each contract in the tx, update state history
                for blob in &blob_tx.blobs {
                    if let Some(contract) = self.contracts.get(&blob.contract_name) {
                        self.state_history
                            .entry(blob.contract_name.clone())
                            .or_default()
                            .push((tx_hash.clone(), contract.clone()));
                    }
                }
            }
        }
    }
}
