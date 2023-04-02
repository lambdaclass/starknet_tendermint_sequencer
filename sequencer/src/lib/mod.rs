use anyhow::{ensure, Result};
use felt::Felt252;
use num_traits::{Num, Zero};
use serde::{Deserialize, Serialize};
use starknet_rs::{
    hash_utils::calculate_contract_address, services::api::contract_class::ContractClass,
    utils::Address,
};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub transaction_type: TransactionType,
    pub transaction_hash: String,
    pub id: String,
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
                let contract_class = ContractClass::try_from(program.as_str())?;
                // This function requires cairo_programs/contracts.json to exist as it uses that cairo program to compute the hash
                let contract_hash = starknet_rs::core::contract_address::starknet_contract_address::compute_class_hash(&contract_class)?;
                Ok(format!(
                    "{}{}",
                    "0x",
                    hex::encode(contract_hash.to_bytes_be())
                ))
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

                let contract_address = calculate_contract_address(
                    &Address((*salt).into()),
                    &felt::Felt252::from_str_radix(&class_hash[2..], 16).unwrap(), // TODO: Handle these errors better
                    &constructor_calldata,
                    Address(Felt252::zero()), // TODO: Deployer address is hardcoded to 0 in starknet-in-rust, ask why
                )?;

                Ok(format!("{}{}", "0x", hex::encode(contract_address.to_bytes_be())))
            }
            TransactionType::Invoke {
                address,
                function,
                inputs,
            } => Ok(format!(
                "Invoked {} with inputs {:?} for contract in address {}",
                function, inputs, address
            )),
        }
    }
}
