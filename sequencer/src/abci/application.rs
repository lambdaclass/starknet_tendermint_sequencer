use anyhow::{anyhow, Result};
use felt::Felt252;
use lib::{Transaction, TransactionType};
use num_traits::Num;
use num_traits::Zero;
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use starknet_rs::business_logic::execution::objects::TransactionExecutionInfo;
use starknet_rs::business_logic::state::state_api::State;
use starknet_rs::business_logic::state::state_api::StateReader;
use starknet_rs::business_logic::transaction::objects::internal_invoke_function::InternalInvokeFunction;
use starknet_rs::business_logic::transaction::transactions::Transaction as StarknetTransaction;
use starknet_rs::business_logic::{
    fact_state::in_memory_state_reader::InMemoryStateReader, state::cached_state::CachedState,
};
use starknet_rs::core::contract_address::starknet_contract_address::compute_class_hash;
use starknet_rs::definitions::general_config::StarknetGeneralConfig;
use starknet_rs::parser_errors::ParserError;
use starknet_rs::serde_structs::contract_abi::read_abi;
use starknet_rs::utils::felt_to_hash;
use starknet_rs::utils::string_to_hash;
use starknet_rs::utils::Address;
use starknet_rs::{
    core::transaction_hash::starknet_transaction_hash::calculate_deploy_transaction_hash,
    hash_utils::calculate_contract_address, services::api::contract_class::ContractClass,
};
use std::path::PathBuf;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Instant,
};
use tendermint_abci::Application;
use tendermint_proto::abci::{
    self, response_process_proposal, RequestPrepareProposal, RequestProcessProposal,
    ResponsePrepareProposal, ResponseProcessProposal,
};
use tracing::{debug, info};

/// An Tendermint ABCI application that works with a Cairo backend.
/// This struct implements the ABCI application hooks, forwarding commands through
/// a channel for the parts that require knowledge of the application state and the Cairo details.
/// For reference see https://docs.tendermint.com/v0.34/introduction/what-is-tendermint.html#abci-overview
#[derive(Debug, Clone)]
pub struct StarknetApp {
    hasher: Arc<Mutex<Sha256>>,
    starknet_state: Arc<Mutex<CachedState<InMemoryStateReader>>>,
    config: StarknetGeneralConfig,
}

// because we don't get a `&mut self` in the ABCI API, we opt to have a mod-level variable
// and because beginblock, endblock and deliver_tx all happen in the same thread, this is safe to do
// an alternative would be Arc<Mutex<>>, but we want to avoid extra-overhead of locks for the benchmark's sake
static mut TRANSACTIONS: usize = 0;
static mut TIMER: Lazy<Instant> = Lazy::new(Instant::now);

impl Application for StarknetApp {
    /// This hook is called once upon genesis. It's used to load a default set of records which
    /// make the initial distribution of credits in the system.
    fn init_chain(&self, _request: abci::RequestInitChain) -> abci::ResponseInitChain {
        info!("Loading genesis");

        Default::default()
    }

    /// This hook provides information about the ABCI application.
    fn info(&self, request: abci::RequestInfo) -> abci::ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            request.version, request.block_version, request.p2p_version
        );

        abci::ResponseInfo {
            data: "cairo-app".to_string(),
            version: "0.1.0".to_string(),
            app_version: 1,
            last_block_height: HeightFile::read_or_create(),

            // using a fixed hash, see the commit() hook
            last_block_app_hash: vec![].into(),
        }
    }

    /// This hook is to query the application for data at the current or past height.
    fn query(&self, _request: abci::RequestQuery) -> abci::ResponseQuery {
        let query_result = Err("Query hook needs implementation");

        match query_result {
            Ok(value) => abci::ResponseQuery {
                value,
                ..Default::default()
            },
            Err(e) => abci::ResponseQuery {
                code: 1,
                log: format!("Error running query: {e}"),
                info: format!("Error running query: {e}"),
                ..Default::default()
            },
        }
    }

    /// This ABCI hook validates an incoming transaction before inserting it in the
    /// mempool and relaying it to other nodes.
    fn check_tx(&self, request: abci::RequestCheckTx) -> abci::ResponseCheckTx {
        let tx: Transaction = bincode::deserialize(&request.tx).unwrap();

        // TODO: Implement starknet validations for each one
        match tx.transaction_type {
            TransactionType::Declare { program: _ } => info!("Received declare transaction"),
            TransactionType::DeployAccount { .. } => info!("Received deploy transaction"),
            TransactionType::Invoke { .. } => info!("Received invoke transaction"),
        }

        abci::ResponseCheckTx {
            ..Default::default()
        }
    }

    /// This hook is called before the app starts processing transactions on a block.
    /// Used to store current proposer and the previous block's voters to assign fees and coinbase
    /// credits when the block is committed.
    fn begin_block(&self, _request: abci::RequestBeginBlock) -> abci::ResponseBeginBlock {
        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        
        unsafe {
            if TRANSACTIONS > 0 {
                info!(
                "{} ms passed between begin_block() calls. {} transactions, {} tps",
                (*TIMER).elapsed().as_millis(),
                TRANSACTIONS,
                (TRANSACTIONS * 1000) as f32 / ((*TIMER).elapsed().as_millis() as f32)
            );
        }
            TRANSACTIONS = 0;

            *TIMER = Instant::now();
        }

        Default::default()
    }

    /// This ABCI hook validates a transaction and applies it to the application state,
    /// for example storing the program verifying keys upon a valid deployment.
    /// Here is also where transactions are indexed for querying the blockchain.
    fn deliver_tx(&self, request: abci::RequestDeliverTx) -> abci::ResponseDeliverTx {
        let tx: Transaction = bincode::deserialize(&request.tx).unwrap();

        // Validation consists of getting the hash and checking whether it is equal
        // to the tx id. The hash executes the program and hashes the trace.

        let tx_hash = tx
            .transaction_type
            .compute_and_hash()
            .map(|x| x == tx.transaction_hash);

        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        unsafe {
            TRANSACTIONS += 1;
        }

        match tx_hash {
            Ok(true) => {
                let _ = self
                    .hasher
                    .lock()
                    .map(|mut hash| hash.update(tx.transaction_hash.clone()));

                // prepare this transaction to be queried by app.tx_id
                let index_event = abci::Event {
                    r#type: "app".to_string(),
                    attributes: vec![abci::EventAttribute {
                        key: "tx_id".to_string(),
                        value: tx.transaction_hash.to_string(), // TODO: Add more useful metadata
                        index: true,
                    }],
                };
                let events = vec![index_event];

                match tx.transaction_type {
                    TransactionType::Declare { program } => {
                        let contract_class = ContractClass::try_from(program.as_str())
                            .expect("Could not load contract from payload");
                        // TODO: Maybe we can get contract_hash as part of the TransactionType and validate it instead of recalculating on each step
                        // This function requires cairo_programs/contracts.json to exist as it uses that cairo program to compute the hash
                        let contract_hash_felt = compute_class_hash(&contract_class).unwrap();
                        let contract_hash = felt_to_hash(&contract_hash_felt);

                        self.starknet_state
                            .lock()
                            .map(|mut state| {
                                state
                                    .set_contract_class(&contract_hash, &contract_class)
                                    .unwrap()
                            })
                            .unwrap();

                        // TODO: Should we send an event about this?
                        info!("Declared tx_id: {}", tx.id);
                        info!("Class Hash 0x{}", hex::encode(contract_hash),);
                    }
                    TransactionType::DeployAccount {
                        class_hash,
                        salt,
                        inputs,
                    } => {
                        let constructor_calldata = match &inputs {
                            Some(vec) => vec.iter().map(|&n| n.into()).collect(),
                            None => Vec::new(),
                        };
                        let address = calculate_contract_address(
                            &Address(salt.into()),
                            &Felt252::from_str_radix(&class_hash[2..], 16).unwrap(), // TODO: Handle these errors better
                            &constructor_calldata,
                            Address(0.into()),
                        )
                        .unwrap();

                        let _ = self.starknet_state.lock().map(|mut state| {
                            // let class = state
                            //     .get_contract_class(&felt_to_hash(&address.clone()))
                            //     .unwrap();
                            state.deploy_contract(
                                Address(address.clone()),
                                string_to_hash(&class_hash),
                            )
                        });

                        let tx_hash = calculate_deploy_transaction_hash(
                            0, // TODO: How are versions handled?
                            &Address(address.clone()),
                            &constructor_calldata,
                            Felt252::zero(),
                        )
                        .unwrap();

                        info!(
                            "Deployed tx_id {}, Address: {}, tx_hash: {}",
                            tx.id, address, tx_hash
                        );
                    }
                    TransactionType::Invoke {
                        address,
                        abi,
                        function,
                        inputs,
                    } => match self.run_invoke_tx(&address, &abi, &function, &inputs) {
                        Ok(result) => {
                            info!(
                                    "Invoked tx_id {}, Address: {}, abi: {}, function: {}, inputs: {:?}",
                                    tx.id,
                                    address,
                                    abi.display(),
                                    function,
                                    inputs,
                                );
                            info!("Result: {:?}", result)
                        }
                        Err(error) => {
                            info!(
                                    "Invoke failed for tx_id {}, Address: {}, abi: {}, function: {}, inputs: {:?}",
                                    tx.id,
                                    address,
                                    abi.display(),
                                    function,
                                    inputs,
                                );
                            info!("Error: {:?}", error)
                        }
                    },
                }

                abci::ResponseDeliverTx {
                    events,
                    data: tx.transaction_hash.into(),
                    ..Default::default()
                }
            }
            Ok(false) => abci::ResponseDeliverTx {
                code: 1,
                log: "Error delivering transaction. Integrity check failed.".to_string(),
                info: "Error delivering transaction. Integrity check failed.".to_string(),
                ..Default::default()
            },
            Err(e) => abci::ResponseDeliverTx {
                code: 1,
                log: format!("Error delivering transaction: {e}"),
                info: format!("Error delivering transaction: {e}"),
                ..Default::default()
            },
        }
    }

    /// Applies validator set updates based on staking transactions included in the block.
    /// For details about validator set update semantics see:
    /// https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/apps.md#endblock
    fn end_block(&self, _request: abci::RequestEndBlock) -> abci::ResponseEndBlock {
        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        unsafe {
            info!(
                "Committing block with {} transactions in {} ms. TPS: {}",
                TRANSACTIONS,
                (*TIMER).elapsed().as_millis(),
                (TRANSACTIONS * 1000) as f32 / ((*TIMER).elapsed().as_millis() as f32)
            );
        }
        abci::ResponseEndBlock {
            ..Default::default()
        }
    }

    /// This hook commits is called when the block is comitted (after deliver_tx has been called for each transaction).
    /// Changes to application should take effect here. Tendermint guarantees that no transaction is processed while this
    /// hook is running.
    /// The result includes a hash of the application state which will be included in the block header.
    /// This hash should be deterministic, different app state hashes will produce blockchain forks.
    /// New credits records are created to assign validator rewards.
    fn commit(&self) -> abci::ResponseCommit {
        // the app hash is intended to capture the state of the application that's not contained directly
        // in the blockchain transactions (as tendermint already accounts for that with other hashes).
        // https://github.com/tendermint/tendermint/issues/1179
        // https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/apps.md#query-proofs

        let app_hash = self
            .hasher
            .lock()
            .map(|hasher| hasher.clone().finalize().as_slice().to_vec());

        let height = HeightFile::increment();

        info!("Committing height {}", height,);

        match app_hash {
            Ok(hash) => abci::ResponseCommit {
                data: hash.into(),
                retain_height: 0,
            },
            // error should be handled here
            _ => abci::ResponseCommit {
                data: vec![].into(),
                retain_height: 0,
            },
        }
    }

    /// A stage where the application can modify the list of transactions
    /// in the preliminary proposal.
    ///
    /// The default implementation implements the required behavior in a
    /// very naive way, removing transactions off the end of the list
    /// until the limit on the total size of the transaction is met as
    /// specified in the `max_tx_bytes` field of the request, or there are
    /// no more transactions. It's up to the application to implement
    /// more elaborate removal strategies.
    ///
    /// This method is introduced in ABCI++.
    fn prepare_proposal(&self, request: RequestPrepareProposal) -> ResponsePrepareProposal {
        // Per the ABCI++ spec: if the size of RequestPrepareProposal.txs is
        // greater than RequestPrepareProposal.max_tx_bytes, the Application
        // MUST remove transactions to ensure that the
        // RequestPrepareProposal.max_tx_bytes limit is respected by those
        // transactions returned in ResponsePrepareProposal.txs.
        let RequestPrepareProposal {
            mut txs,
            max_tx_bytes,
            ..
        } = request;
        let max_tx_bytes: usize = max_tx_bytes.try_into().unwrap_or(0);
        let mut total_tx_bytes: usize = txs
            .iter()
            .map(|tx| tx.len())
            .fold(0, |acc, len| acc.saturating_add(len));
        while total_tx_bytes > max_tx_bytes {
            if let Some(tx) = txs.pop() {
                total_tx_bytes = total_tx_bytes.saturating_sub(tx.len());
            } else {
                break;
            }
        }
        ResponsePrepareProposal { txs }
    }

    /// A stage where the application can accept or reject the proposed block.
    ///
    /// The default implementation returns the status value of `ACCEPT`.
    ///
    /// This method is introduced in ABCI++.
    fn process_proposal(&self, _request: RequestProcessProposal) -> ResponseProcessProposal {
        ResponseProcessProposal {
            status: response_process_proposal::ProposalStatus::Accept as i32,
        }
    }
}

impl StarknetApp {
    /// Constructor.
    pub fn new() -> Self {
        let mut state = CachedState::new(InMemoryStateReader::default(), Some(HashMap::new()));

        state.set_contract_classes(Default::default()).unwrap();
        let new_state = Self {
            hasher: Arc::new(Mutex::new(Sha256::new())),
            starknet_state: Arc::new(Mutex::new(CachedState::new(
                InMemoryStateReader::default(),
                Some(HashMap::new()),
            ))),
            config: StarknetGeneralConfig::default(),
        };

        let height_file = HeightFile::read_or_create();

        info!(
            "Starting with Starknet State: {:?}. Height file has value: {}",
            new_state.starknet_state, height_file
        );
        new_state
    }

    fn run_invoke_tx(
        &self,
        address: &String,
        abi: &PathBuf,
        function: &String,
        inputs: &Option<Vec<i32>>,
    ) -> Result<TransactionExecutionInfo> {
        let contract_address = Address(
            Felt252::from_str_radix(&address[2..], 16)
                .map_err(|_| anyhow!("Could not parse address: {}", address))?,
        );
        let calldata = match &inputs {
            Some(vec) => vec.iter().map(|&n| n.into()).collect(),
            None => Vec::new(),
        };
        let result = self.starknet_state.lock().map(|mut state| {
            let class_hash = *state.get_class_hash_at(&contract_address).unwrap();
            let contract_class = state
                .get_contract_class(&class_hash)
                .map_err(|_| anyhow!("No contract class found for class hash: {:?}", &class_hash))
                .unwrap();

            let function_entrypoint_indexes = read_abi(abi);

            let entry_points_by_type = contract_class.entry_points_by_type().clone();
            let (entry_point_index, entry_point_type) = function_entrypoint_indexes
                .get(function)
                .ok_or_else(|| ParserError::FunctionEntryPoint(function.clone()))
                .unwrap();

            let entry_point_selector = entry_points_by_type
                .get(entry_point_type)
                .ok_or(ParserError::EntryPointType(*entry_point_type))
                .unwrap()
                .get(*entry_point_index)
                .ok_or(ParserError::EntryPointIndex(*entry_point_index))
                .unwrap()
                .selector()
                .clone();

            let tx = InternalInvokeFunction::new(
                contract_address,
                entry_point_selector,
                0,
                calldata,
                vec![],
                0.into(),
                Some(0.into()),
            )
            .unwrap();

            let tx = StarknetTransaction::InvokeFunction(tx);
            let mut state = state.clone();

            tx.execute(&mut state, &self.config).unwrap()
        });

        result.map_err(|_| anyhow!("Error running invoke_tx"))
    }
}

/// Local file used to track the last block height seen by the abci application.
struct HeightFile;

impl HeightFile {
    const PATH: &str = "abci.height";

    fn read_or_create() -> i64 {
        // if height file is missing or unreadable, create a new one from zero height
        if let Ok(bytes) = std::fs::read(Self::PATH) {
            // if contents are not readable, crash intentionally
            bincode::deserialize(&bytes).expect("Contents of height file are not readable")
        } else {
            std::fs::write(Self::PATH, bincode::serialize(&0i64).unwrap()).unwrap();
            0i64
        }
    }

    fn increment() -> i64 {
        // if the file is missing or contents are unexpected, we crash intentionally;
        let mut height: i64 = bincode::deserialize(&std::fs::read(Self::PATH).unwrap()).unwrap();
        height += 1;
        std::fs::write(Self::PATH, bincode::serialize(&height).unwrap()).unwrap();
        height
    }
}

// just covering a few special cases here. lower level test are done in record store and program store, higher level in integration tests.
#[cfg(test)]
mod tests {
    fn _test_hook() {}
}
