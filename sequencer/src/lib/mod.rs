use std::collections::HashMap;

use anyhow::{ensure, Result};
use felt::Felt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use starknet_rs::business_logic::execution::execution_entry_point::ExecutionEntryPoint;
use starknet_rs::business_logic::execution::objects::{CallType, TransactionExecutionContext};
use starknet_rs::business_logic::fact_state::contract_state::ContractState;
use starknet_rs::business_logic::fact_state::in_memory_state_reader::InMemoryStateReader;
use starknet_rs::business_logic::fact_state::state::ExecutionResourcesManager;
use starknet_rs::business_logic::state::cached_state::CachedState;
use starknet_rs::definitions::general_config::StarknetGeneralConfig;
use starknet_rs::services::api::contract_class::{ContractClass, EntryPointType};
use starknet_rs::utils::Address;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub id: String,
    pub transaction_hash: String, // this acts
    pub transaction_type: TransactionType,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    /// Create new contract class.
    Declare { program: String },

    /// Create an instance of a contract which will have storage assigned. (Accounts are a contract themselves)
    Deploy,

    /// Execute a function from a deployed contract.
    Invoke,

    // TODO: Remove this when other transactions are implemented
    FunctionExecution {
        function: String,
        program_name: String,
    },
}

impl Transaction {
    pub fn with_type(transaction_type: TransactionType) -> Result<Transaction> {
        Ok(Transaction {
            transaction_hash: transaction_type.compute_and_hash()?,
            transaction_type,
            id: Uuid::new_v4().to_string(),
        })
    }

    /// Verify that the transaction id is consistent with its contents, by checking its sha256 hash.
    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.transaction_hash == self.transaction_type.compute_and_hash()?,
            "Corrupted transaction: Inconsistent transaction id"
        );

        Ok(())
    }
}

impl TransactionType {
    pub fn compute_and_hash(&self) -> Result<String> {
        match self {
            TransactionType::FunctionExecution {
                function,
                program_name: _,
            } => {
                let general_config = StarknetGeneralConfig::default();

                let tx_execution_context = TransactionExecutionContext::create_for_testing(
                    Address(0.into()),
                    10,
                    0.into(),
                    general_config.invoke_tx_max_n_steps(),
                    1,
                );

                let contract_address = Address(1111.into());
                let class_hash = [1; 32];
                let program = include_str!("../../programs/fibonacci.json");
                let contract_class = ContractClass::try_from(program.to_string())
                    .expect("Could not load contract from JSON");

                let contract_state = ContractState::new(
                    class_hash,
                    tx_execution_context.nonce().clone(),
                    Default::default(),
                );
                let mut state_reader = InMemoryStateReader::new(HashMap::new(), HashMap::new());
                state_reader
                    .contract_states_mut()
                    .insert(contract_address.clone(), contract_state);

                let mut state = CachedState::new(
                    state_reader,
                    Some([(class_hash, contract_class)].into_iter().collect()),
                );

                let entry_point = ExecutionEntryPoint::new(
                    contract_address,
                    vec![],
                    Felt::from_bytes_be(&starknet_rs::utils::calculate_sn_keccak(
                        function.as_bytes(),
                    )),
                    Address(0.into()),
                    EntryPointType::External,
                    CallType::Delegate.into(),
                    class_hash.into(),
                );

                let mut resources_manager = ExecutionResourcesManager::default();

                entry_point
                    .execute(
                        &mut state,
                        &general_config,
                        &mut resources_manager,
                        &tx_execution_context,
                    )
                    .expect("Could not execute contract");

                let mut hasher = Sha256::new();
                hasher.update(function);
                let hash = hasher.finalize().as_slice().to_owned();
                Ok(hex::encode(hash))
            }
            TransactionType::Declare { program } => {
                let contract_class = ContractClass::try_from(program.to_string())
                    .expect("Could not load contract from JSON");
                // This function requires cairo_programs/contracts.json to exist as it uses that cairo program to compute the hash
                let contract_hash = starknet_rs::core::contract_address::starknet_contract_address::compute_class_hash(&contract_class).unwrap();
                Ok(hex::encode(contract_hash.to_bytes_be()))
            }
            TransactionType::Deploy => todo!(),
            TransactionType::Invoke => todo!(),
        }
    }
}
