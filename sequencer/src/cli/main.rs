use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use lib::{Transaction, TransactionType};
use std::fs;
use std::path::PathBuf;
use std::str;
use tendermint_rpc::{Client, HttpClient};
use tracing::debug;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const LOCAL_SEQUENCER_URL: &str = "http://127.0.0.1:26657";

#[derive(Parser)]
struct Cli {
    /// Subcommand to execute
    #[command(subcommand)]
    command: Command,

    /// Output log lines to stdout based on the desired log level (RUST_LOG env var).
    #[clap(short, long, global = false, default_value_t = false)]
    pub verbose: bool,

    /// Just run the program and return the transaction in the stdio
    #[clap(short, long, global = false, default_value_t = false)]
    pub no_broadcast: bool,

    /// Tendermint node url
    #[clap(short, long, env = "SEQUENCER_URL", default_value = LOCAL_SEQUENCER_URL)]
    pub url: String,
}

#[derive(Subcommand)]
enum Command {
    Declare(DeclareArgs),
    DeployAccount(DeployArgs),
    Invoke(InvokeArgs),
}

#[derive(Args)]
pub struct DeclareArgs {
    #[arg(long)]
    contract: PathBuf,
}

#[derive(Args)]
pub struct DeployArgs {
    #[arg(long = "class_hash")]
    class_hash: String,
    // TODO: randomize salt by default?
    #[arg(long, default_value = "1111")]
    salt: i32,
    #[arg(long, num_args=1.., value_delimiter = ' ')]
    inputs: Option<Vec<i32>>,
}

#[derive(Args, Debug)]
pub struct InvokeArgs {
    /// Contract Address
    #[clap(short, long)]
    address: String,

    /// ABI
    #[clap(long)]
    abi: PathBuf,

    /// Function name
    #[clap(short, long)]
    function: String,

    /// Function input values
    #[clap(long, num_args=1.., value_delimiter = ' ')]
    inputs: Option<Vec<i32>>,

    /// tendermint node url
    #[clap(short, long, env = "SEQUENCER_URL", default_value = LOCAL_SEQUENCER_URL)]
    pub url: String,
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

    // TODO: Have these functions return a Result and handle errors here
    let (exit_code, output) = match cli.command {
        Command::Declare(declare_args) => do_declare(declare_args, &cli.url).await,
        Command::DeployAccount(deploy_args) => do_deploy(deploy_args, &cli.url).await,
        Command::Invoke(invoke_args) => do_invoke(invoke_args, &cli.url).await,
    };

    println!("{output:#}");
    std::process::exit(exit_code);
}

async fn do_declare(args: DeclareArgs, url: &str) -> (i32, String) {
    let program = fs::read_to_string(args.contract).unwrap();
    let transaction_type = TransactionType::Declare { program };
    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    match broadcast(transaction_serialized, url).await {
        Ok(_) => (0, "DECLARE: Sent transaction".to_string()),
        Err(e) => (1, format!("DECLARE: Error sending out transaction: {e}")),
    }
}

async fn do_deploy(args: DeployArgs, url: &str) -> (i32, String) {
    let transaction_type = TransactionType::DeployAccount {
        class_hash: args.class_hash,
        salt: args.salt,
        inputs: args.inputs,
    };

    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    match broadcast(transaction_serialized, url).await {
        Ok(_) => (
            0,
            format!("DEPLOY: Sent transaction - ID: {}", transaction.id),
        ),
        Err(e) => (1, format!("DEPLOY: Error sending out transaction: {e}")),
    }
}

async fn do_invoke(args: InvokeArgs, _url: &str) -> (i32, String) {
    (0, format!("args: {:?}", args))
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
