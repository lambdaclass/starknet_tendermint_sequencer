use anyhow::{ensure, Result};
use std::path::PathBuf;

use num_traits::Num;
use serde::{Deserialize, Serialize};
use starknet_rs::{
    hash_utils::calculate_contract_address, services::api::contract_class::ContractClass,
    utils::Address,
};
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
    DeployAccount {
        class_hash: String,
        salt: i32,
        inputs: Option<Vec<i32>>,
    },

    /// Execute a function from a deployed contract.
    Invoke {
        address: String,
        abi: PathBuf,
        function: String,
        inputs: Option<Vec<i32>>,
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
    pub fn assert_integrity(&self) -> Result<()> {
        ensure!(
            self.transaction_hash == self.transaction_type.compute_and_hash()?,
            "Corrupted transaction: Inconsistent transaction id"
        );

        Ok(())
    }
}

impl TransactionType {
    // TODO: Rename this and/or structure the code differently
    pub fn compute_and_hash(&self) -> Result<String> {
        match self {
            TransactionType::Declare { program } => {
                let contract_class = ContractClass::try_from(program.as_str())
                    .expect("Could not load contract from JSON");
                // This function requires cairo_programs/contracts.json to exist as it uses that cairo program to compute the hash
                let contract_hash = starknet_rs::core::contract_address::starknet_contract_address::compute_class_hash(&contract_class).unwrap();
                Ok(hex::encode(contract_hash.to_bytes_be()))
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
                    &Address((*salt).into()),
                    &felt::Felt::from_str_radix(&class_hash[2..], 16).unwrap(), // TODO: Handle these errors better
                    &constructor_calldata,
                    Address(0.into()),
                )
                .unwrap();

                Ok(hex::encode(address.to_bytes_be()))
            }
            TransactionType::Invoke {
                address: _,
                abi: _,
                function: _,
                inputs: _,
            } => Ok("Not yet implmented - working on it".to_owned()),
        }
    }
}
