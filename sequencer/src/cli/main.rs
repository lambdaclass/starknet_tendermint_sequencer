use crate::tendermint::broadcast;
use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use lib::{Transaction, TransactionType};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::str;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub mod tendermint;
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
    Get(GetArgs),
}

#[derive(Args)]
pub struct GetArgs {
    transaction_id: String,
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

    let result = match cli.command {
        Command::Declare(declare_args) => do_declare(declare_args, &cli.url).await,
        Command::DeployAccount(deploy_args) => do_deploy(deploy_args, &cli.url).await,
        Command::Invoke(invoke_args) => do_invoke(invoke_args, &cli.url).await,
        Command::Get(get_args) => {
            tendermint::get_transaction(&get_args.transaction_id, &cli.url).await
        }
    };

    let (code, output) = match result {
        Ok(output) => (0, json!(output)),
        Err(err) => (1, json!({"error": err.to_string()})),
    };

    println!("{output:#}");
    std::process::exit(code);
}

async fn do_declare(args: DeclareArgs, url: &str) -> Result<Transaction> {
    let program = fs::read_to_string(args.contract).unwrap();
    let transaction_type = TransactionType::Declare { program };
    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    match tendermint::broadcast(transaction_serialized, url).await {
        Ok(_) => Ok(transaction),
        Err(e) => bail!("DECLARE: Error ocurred when sending out transaction: {e}"),
    }
}

async fn do_deploy(args: DeployArgs, url: &str) -> Result<Transaction> {
    let transaction_type = TransactionType::DeployAccount {
        class_hash: args.class_hash,
        salt: args.salt,
        inputs: args.inputs,
    };

    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    match tendermint::broadcast(transaction_serialized, url).await {
        Ok(_) => Ok(transaction),
        Err(e) => bail!("DEPLOY: Error sending out transaction: {e}"),
    }
}

async fn do_invoke(args: InvokeArgs, url: &str) -> Result<Transaction> {
    let transaction_type = TransactionType::Invoke {
        address: args.address,
        abi: args.abi,
        function: args.function,
        inputs: args.inputs,
    };

    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();

    match broadcast(transaction_serialized, url).await {
        Ok(_) => Ok(transaction),
        Err(e) => bail!("INVOKE: Error sending out transaction: {e}"),
    }
}
