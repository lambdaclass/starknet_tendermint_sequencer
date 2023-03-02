use anyhow::{ensure, Context, Result};
use cairo_vm::{
    hint_processor::builtin_hint_processor::builtin_hint_processor_definition::BuiltinHintProcessor,
    types::{program::Program, relocatable::MaybeRelocatable},
    vm::{runners::cairo_runner::CairoRunner, vm_core::VirtualMachine},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use tracing::info;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Transaction {
    pub id: String,
    pub transaction_hash: String, // this acts
    pub transaction_type: TransactionType,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransactionType {
    FunctionExecution {
        program: String,
        function: String,
        program_name: String,
        enable_trace: bool,
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
        let mut hasher = Sha256::new();

        match self {
            TransactionType::FunctionExecution {
                program: program_str,
                function,
                program_name: _,
                enable_trace: execute_trace,
            } => {
                let program = Program::from_reader(program_str.as_bytes(), None)?;
                let mut vm = VirtualMachine::new(*execute_trace);

                let mut cairo_runner = CairoRunner::new(&program, "all", false)?;

                let mut hint_processor = BuiltinHintProcessor::new_empty();

                let entrypoint = program
                    .identifiers
                    .get(&format!("__main__.{function}"))
                    .and_then(|x| x.pc)
                    .context("Error geting entrypoint function")?;

                cairo_runner.initialize_builtins(&mut vm)?;
                cairo_runner.initialize_segments(&mut vm, None);

                cairo_runner.run_from_entrypoint(
                    entrypoint,
                    &[
                        &MaybeRelocatable::from(2).into(),
                        &MaybeRelocatable::from((2, 0)).into(),
                    ],
                    false,
                    &mut vm,
                    &mut hint_processor,
                )?;
                cairo_runner.relocate(&mut vm).unwrap();

                let trace = cairo_runner.relocated_trace;

                match trace {
                    Some(trace) => {
                        for reg in trace {
                            hasher.update(serde_json::to_string(&reg)?);
                        }
                    }
                    None => info!("Trace not enabled, not executing/hashing"),
                }
                hasher.update(function);
            }
        }

        let hash = hasher.finalize().as_slice().to_owned();
        Ok(hex::encode(hash))
    }
}
