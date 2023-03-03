use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use lib::{Transaction, TransactionType};
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use tendermint_abci::Application;
use tendermint_proto::abci;
use starknet_rs::testing::starknet_state::StarknetState;

use tracing::{debug, info};

/// An Tendermint ABCI application that works with a Cairo backend.
/// This struct implements the ABCI application hooks, forwarding commands through
/// a channel for the parts that require knowledge of the application state and the Cairo details.
/// For reference see https://docs.tendermint.com/v0.34/introduction/what-is-tendermint.html#abci-overview
#[derive(Debug, Clone)]
pub struct CairoApp {
    hasher: Arc<Mutex<Sha256>>,
    starknet_state: StarknetState
}

// because we don't get a `&mut self` in the ABCI API, we opt to have a mod-level variable
// and because beginblock, endblock and deliver_tx all happen in the same thread, this is safe to do 
// an alternative would be Arc<Mutex<>>, but we want to avoid extra-overhead of locks for the benchmark's sake
static mut TRANSACTIONS: usize = 0;
static mut TIMER: Lazy<Instant> = Lazy::new(Instant::now);

impl Application for CairoApp {
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
            last_block_app_hash: vec![],
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

        match tx.transaction_type {
            TransactionType::FunctionExecution {
                program: _,
                function,
                program_name,
                enable_trace: _,
            } => {
                info!(
                    "Received execution transaction. Function: {}, program {}",
                    function, program_name
                );
            }
            TransactionType::Declare => todo!(),
            TransactionType::Deploy => todo!(),
            TransactionType::Invoke => todo!(),
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
            TRANSACTIONS = 0;

            info!(
                "{} ms passed between previous begin_block() and current begin_block()",
                (*TIMER).elapsed().as_millis()
            );

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
                        key: "tx_id".to_string().into_bytes(),
                        value: tx.transaction_hash.to_string().into_bytes(),
                        index: true,
                    }],
                };
                let mut events = vec![index_event];

                match tx.transaction_type {
                    TransactionType::FunctionExecution {
                        program: _program,
                        function,
                        program_name: _,
                        enable_trace: _,
                    } => {
                        let function_event = abci::Event {
                            r#type: "function".to_string(),
                            attributes: vec![abci::EventAttribute {
                                key: "function".to_string().into_bytes(),
                                value: function.into_bytes(),
                                index: true,
                            }],
                        };
                        events.push(function_event);
                    }
                    TransactionType::Declare => todo!(),
                    TransactionType::Deploy => todo!(),
                    TransactionType::Invoke => todo!(),
                }

                abci::ResponseDeliverTx {
                    events,
                    data: tx.transaction_hash.into_bytes(),
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
                data: hash,
                retain_height: 0,
            },
            // error should be handled here
            _ => abci::ResponseCommit {
                data: vec![],
                retain_height: 0,
            },
        }
    }
}

impl CairoApp {
    /// Constructor.
    pub fn new() -> Self {
        let new_state = Self {
            hasher: Arc::new(Mutex::new(Sha256::new())),
            starknet_state: StarknetState::new(None)
        };

        info!("Starting with Starknet State: {:?}", new_state.starknet_state);
        new_state
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
