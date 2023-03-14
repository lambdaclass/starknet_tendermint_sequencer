use once_cell::sync::Lazy;
use starknet_rs::testing::starknet_state::StarknetState;
use tracing::{debug, info};
use std::{sync::Arc, sync::Mutex,time::Instant};
use sha2::{Digest, Sha256};
use crate::transaction::{Transaction, TransactionType};

use abci::{
    async_api::{
        Consensus as ConsensusTrait, Info as InfoTrait, Mempool as MempoolTrait,
        Snapshot as SnapshotTrait,
    },
    async_trait,
    types::*,
};


/// The app's state, containing a Revm DB.
// TODO: Should we instead try to replace this with Anvil and implement traits for it?
#[derive(Clone, Debug)]
pub struct State {
    pub block_height: i64,
    pub app_hash: Vec<u8>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            block_height: 0,
            app_hash: Vec::new(),
        }
    }
}

// because we don't get a `&mut self` in the ABCI API, we opt to have a mod-level variable
// and because beginblock, endblock and deliver_tx all happen in the same thread, this is safe to do
// an alternative would be Arc<Mutex<>>, but we want to avoid extra-overhead of locks for the benchmark's sake
static mut TRANSACTIONS: usize = 0;
static mut TIMER: Lazy<Instant> = Lazy::new(Instant::now);

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TransactionResult {
    gas: u64,
}

pub struct Consensus {
    pub committed_state: Arc<Mutex<State>>,
    pub current_state: Arc<Mutex<State>>,
    hasher: Arc<Mutex<Sha256>>,
    starknet_state: StarknetState,
}

impl Consensus {
    pub fn new(state: State) -> Self {
        let committed_state = Arc::new(Mutex::new(state.clone()));
        let current_state = Arc::new(Mutex::new(state));

        Consensus {
            committed_state,
            current_state,
            hasher: Arc::new(Mutex::new(Sha256::new())),
            starknet_state: StarknetState::new(None),
        }
    }
}

#[async_trait]
impl ConsensusTrait for Consensus {
    #[tracing::instrument(skip(self))]
    async fn init_chain(&self, _init_chain_request: RequestInitChain) -> ResponseInitChain {
        ResponseInitChain::default()
    }

    #[tracing::instrument(skip(self))]
    async fn begin_block(&self, _begin_block_request: RequestBeginBlock) -> ResponseBeginBlock {
        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        unsafe {
            info!(
                "{} ms passed between begin_block() calls. {} transactions, {} tps",
                (*TIMER).elapsed().as_millis(),
                TRANSACTIONS,
                (TRANSACTIONS * 1000) as f32 / ((*TIMER).elapsed().as_millis() as f32)
            );
            TRANSACTIONS = 0;

            *TIMER = Instant::now();
        }

        Default::default()
    }

    #[tracing::instrument(skip(self))]
    async fn deliver_tx(&self, request: RequestDeliverTx) -> ResponseDeliverTx {
        tracing::trace!("delivering tx");

        let tx: Transaction = serde_json::from_slice(&request.tx).unwrap();

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
                let index_event = Event {
                    r#type: "app".to_string(),
                    attributes: vec![EventAttribute {
                        key: "tx_id".to_string().into_bytes(),
                        value: tx.transaction_hash.to_string().into_bytes(),
                        index: true,
                    }],
                };
                let mut events = vec![index_event];

                match tx.transaction_type {
                    TransactionType::FunctionExecution {
                        function,
                        program_name: _,
                    } => {
                        let function_event = Event {
                            r#type: "function".to_string(),
                            attributes: vec![EventAttribute {
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

                ResponseDeliverTx {
                    events,
                    data: tx.transaction_hash.into_bytes(),
                    ..Default::default()
                }
            }
            Ok(false) => ResponseDeliverTx {
                code: 1,
                log: "Error delivering transaction. Integrity check failed.".to_string(),
                info: "Error delivering transaction. Integrity check failed.".to_string(),
                ..Default::default()
            },
            Err(e) => ResponseDeliverTx {
                code: 1,
                log: format!("Error delivering transaction: {e}"),
                info: format!("Error delivering transaction: {e}"),
                ..Default::default()
            },
        }
    }

    #[tracing::instrument(skip(self))]
    async fn end_block(&self, _end_block_request: RequestEndBlock) -> ResponseEndBlock {
        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        unsafe {
            info!(
                "Committing block with {} transactions in {} ms. TPS: {}",
                TRANSACTIONS,
                (*TIMER).elapsed().as_millis(),
                (TRANSACTIONS * 1000) as f32 / ((*TIMER).elapsed().as_millis() as f32)
            );
        }
        ResponseEndBlock {
            ..Default::default()
        }
    }

    #[tracing::instrument(skip(self))]
    async fn commit(&self, _commit_request: RequestCommit) -> ResponseCommit {

        let app_hash: Result<Vec<u8>, String> = Ok(vec![]);

        // because begin_block, [deliver_tx] and end_block/commit are on the same thread, this is safe to do (see declaration of statics)
        unsafe {
            info!(
                "Committing block with {} transactions in {} ms. TPS: {}",
                TRANSACTIONS,
                (*TIMER).elapsed().as_millis(),
                (TRANSACTIONS * 1000) as f32 / ((*TIMER).elapsed().as_millis() as f32)
            );
        }

        match app_hash {
            Ok(hash) => ResponseCommit {
                data: hash,
                retain_height: 0,
            },
            // error should be handled here
            _ => ResponseCommit {
                data: vec![],
                retain_height: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Mempool;

#[async_trait]
impl MempoolTrait for Mempool {
    async fn check_tx(&self, _check_tx_request: RequestCheckTx) -> ResponseCheckTx {
        ResponseCheckTx::default()
    }
}

#[derive(Debug, Clone)]
pub struct Info {
    pub state: Arc<Mutex<State>>,
}

#[async_trait]
impl InfoTrait for Info {
    // replicate the eth_call interface
    async fn query(&self, query_request: RequestQuery) -> ResponseQuery {
        ResponseQuery {
            key: query_request.data,
            ..Default::default()
        }
    }

    async fn info(&self, info_request: RequestInfo) -> ResponseInfo {
        debug!(
            "Got info request. Tendermint version: {}; Block version: {}; P2P version: {}",
            info_request.version, info_request.block_version, info_request.p2p_version
        );

        ResponseInfo {
            data: "cairo-app".to_string(),
            version: "0.1.0".to_string(),
            app_version: 1,
            last_block_height: 0i64,

            // using a fixed hash, see the commit() hook
            last_block_app_hash: vec![],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot;

impl SnapshotTrait for Snapshot {}


/* 
#[cfg(test)]
mod tests {
    use super::*;
    // use ethers::prelude::*;

    #[tokio::test]
    async fn run_and_query_tx() {
        let val = ethers::utils::parse_units(1, 18).unwrap();
        let alice = Address::random();
        let bob = Address::random();

        let mut state = State::default();

        // give alice some money
        state.db.insert_account_info(
            alice,
            revm::AccountInfo {
                balance: val,
                ..Default::default()
            },
        );

        // make the tx
        let tx = TransactionRequest::new()
            .from(alice)
            .to(bob)
            .gas_price(0)
            .data(vec![1, 2, 3, 4, 5])
            .gas(31000)
            .value(val);

        // Send it over an ABCI message

        let consensus = Consensus::new(state);

        let req = RequestDeliverTx {
            tx: serde_json::to_vec(&tx).unwrap(),
        };
        let res = consensus.deliver_tx(req).await;
        let res: TransactionResult = serde_json::from_slice(&res.data).unwrap();
        // tx passed
        assert_eq!(res.exit, Return::Stop);

        // now we query the state for bob's balance
        let info = Info {
            state: consensus.current_state.clone(),
        };
        let res = info
            .query(RequestQuery {
                data: serde_json::to_vec(&Query::Balance(bob)).unwrap(),
                ..Default::default()
            })
            .await;
        let res: QueryResponse = serde_json::from_slice(&res.value).unwrap();
        let balance = res.as_balance();
        assert_eq!(balance, val);
    }
} */
