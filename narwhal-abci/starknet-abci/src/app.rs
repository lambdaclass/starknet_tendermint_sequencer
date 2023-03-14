
use crate::{Consensus, Info, Mempool, Snapshot, State};
use std::sync::{Arc, Mutex};

pub struct App {
    pub mempool: Mempool,
    pub snapshot: Snapshot,
    pub consensus: Consensus,
    pub info: Info,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// demo
impl App {
    pub fn new() -> Self {
        let state = State {
            block_height: Default::default(),
            app_hash: Default::default(),
        };

        let committed_state = Arc::new(Mutex::new(state.clone()));

        let consensus = Consensus::new(state);
        let mempool = Mempool::default();

        let info = Info {
            state: committed_state,
        };
        let snapshot = Snapshot::default();

        App {
            consensus,
            mempool,
            info,
            snapshot,
        }
    }
}
