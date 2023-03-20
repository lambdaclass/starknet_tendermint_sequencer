use anyhow::{anyhow, bail, Result};
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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Execute(ExecuteArgs),
    Declare(DeclareArgs),
    Deploy(DeployArgs),
    Invoke(InvokeArgs),
}

#[derive(Args)]
pub struct ExecuteArgs {
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
}

#[derive(Args)]
pub struct DeclareArgs {
    #[arg(long)]
    contract: PathBuf,
}

#[derive(Args)]
pub struct DeployArgs {}

#[derive(Args)]
pub struct InvokeArgs {}

#[tokio::main()]
async fn main() {
    let cli = Cli::parse();

    let (exit_code, output) = match cli.command {
        Commands::Execute(execute_args) => do_execute(execute_args).await,
        Commands::Declare(declare_args) => do_declare(declare_args).await,
        Commands::Deploy(deploy_args) => do_deploy(deploy_args).await,
        Commands::Invoke(invoke_args) => do_invoke(invoke_args).await,
    };

    println!("{output:#}");
    std::process::exit(exit_code);
}

async fn do_execute(args: ExecuteArgs) -> (i32, String) {
    if args.verbose {
        tracing_subscriber::fmt()
            // Use a more compact, abbreviated log format
            .compact()
            .with_env_filter(EnvFilter::from_default_env())
            // Build and init the subscriber
            .finish()
            .init();
    }

    match run(
        &args.path,
        &args.function_name,
        &args.url,
        args.no_broadcast,
    )
    .await
    {
        Ok(output) => (0, output),
        Err(err) => (1, format!("error: {err}")),
    }
}

async fn do_declare(args: DeclareArgs) -> (i32, String) {
    let program = fs::read_to_string(args.contract).unwrap();
    let transaction_type = TransactionType::Declare { program };
    let transaction = Transaction::with_type(transaction_type).unwrap();
    let transaction_serialized = bincode::serialize(&transaction).unwrap();
    match broadcast(transaction_serialized, LOCAL_SEQUENCER_URL).await {
        Ok(_) => (0, format!("DECLARE: Sent transaction")),
        Err(e) => (1, format!("DECLARE: Error sending out transaction: {}", e)),
    }
}

async fn do_deploy(_args: DeployArgs) -> (i32, String) {
    todo!()
}

async fn do_invoke(_args: InvokeArgs) -> (i32, String) {
    todo!()
}

async fn run(
    path: &PathBuf,
    function_name: &str,
    sequencer_url: &str,
    no_broadcast: bool,
) -> Result<String> {
    let _program = fs::read_to_string(path)?;

    let transaction_type = TransactionType::FunctionExecution {
        function: function_name.to_owned(),
        program_name: path
            .file_name()
            .expect("Error getting file name")
            .to_string_lossy()
            .to_string(),
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
