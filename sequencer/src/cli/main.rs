use anyhow::{anyhow, bail, Result};
use clap::Parser;
use lib::{Transaction, TransactionType};
use std::fs;
use std::path::PathBuf;
use std::str;
use tendermint_rpc::{Client, HttpClient};
use tracing::debug;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const LOCAL_SEQUENCER_URL: &str = "http://127.0.0.1:26657";

#[derive(Debug, Parser)]
#[clap()]
pub struct Cli {
    /// Specify a subcommand.
    #[clap()]
    pub path: PathBuf,

    /// Function name from the compiled Cairo program.
    #[clap()]
    pub function_name: String,

    /// Output log lines to stdout based on the desired log level (RUST_LOG env var).
    #[clap(short, long, global = false, default_value_t = false)]
    pub verbose: bool,

    /// Just run the program and return the transaction in the stdio
    #[clap(short, long, global = false, default_value_t = false)]
    pub no_broadcast: bool,

    /// tendermint node url
    #[clap(short, long, env = "SEQUENCER_URL", default_value = LOCAL_SEQUENCER_URL)]
    pub url: String,

    #[clap(short, long, global = false, default_value_t = false)]
    pub send_to_prover: bool,
}

#[tokio::main()]
async fn main() {
    let cli = Cli::parse();

    if cli.verbose {
        tracing_subscriber::fmt()
            // Use a more compact, abbreviated log format
            .compact()
            .with_env_filter(EnvFilter::from_default_env())
            // Build and init the subscriber
            .finish()
            .init();
    }

    let (exit_code, output) = match run(
        &cli.path,
        &cli.function_name,
        &cli.url,
        cli.no_broadcast,
        cli.send_to_prover,
    )
    .await
    {
        Ok(output) => (0, output),
        Err(err) => (1, format!("error: {err}")),
    };

    println!("{output:#}");
    std::process::exit(exit_code);
}

async fn run(
    path: &PathBuf,
    function_name: &str,
    sequencer_url: &str,
    no_broadcast: bool,
    send_to_prover: bool,
) -> Result<String> {
    let _program = fs::read_to_string(path)?;
    let program = fs::read_to_string(path)?;
    let function = function_name.to_owned();
    let program_name = path
        .file_name()
        .expect("Error getting file name")
        .to_string_lossy()
        .to_string();

    let transaction_type = match send_to_prover {
        false => TransactionType::FunctionExecution {
            function,
            program_name,
        },
        true => TransactionType::FunctionExecutionProver {
            program,
            function,
            program_name,
        },
    };

    let transaction = Transaction::with_type(transaction_type)?;

    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    if no_broadcast {
        Ok(str::from_utf8(&transaction_serialized).unwrap().to_string())
    } else {
        match broadcast(transaction_serialized, sequencer_url).await {
            Ok(_) => Ok(format!(
                "Sent transaction (ID {}) succesfully. Hash: {}",
                transaction.id, transaction.transaction_hash
            )),
            Err(e) => Err(anyhow!("Error sending out transaction: {}", e)),
        }
    }
}

pub async fn broadcast(transaction: Vec<u8>, url: &str) -> Result<()> {
    let client = HttpClient::new(url).unwrap();

    let response = client.broadcast_tx_sync(transaction).await?;

    debug!("Response from CheckTx: {:?}", response);
    match response.code {
        tendermint::abci::Code::Ok => Ok(()),
        tendermint::abci::Code::Err(code) => {
            bail!("Error executing transaction {}: {}", code, response.log)
        }
    }
}
