use clap::Parser;
use lib::{Transaction, TransactionType};
use std::net::SocketAddr;
use std::time::Instant;
use tendermint_rpc::{Client, HttpClient};
use tracing::{info, metadata::LevelFilter};
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

#[derive(Parser)]
#[clap()]
pub struct Cli {
    /// Amount of concurrent threads from which transactions will be sent. Each thread will have a client connection to the network.
    #[clap(short, long, value_parser, value_name = "UINT", default_value_t = 4)]
    pub threads: i32,

    /// Number of transactions per second each thread sends out.
    #[clap(short, long, value_parser, value_name = "UINT", default_value_t = 1000)]
    pub transactions_per_thread: usize,

    /// Nodes to which transactions will be sent to (round-robin).
    #[clap(
        long,
        value_parser,
        value_name = "ADDR",
        use_value_delimiter = true,
        value_delimiter = ' '
    )]
    nodes: Vec<SocketAddr>,
}

#[tokio::main()]
async fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        // Use a more compact, abbreviated log format
        .compact()
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        .with_max_level(LevelFilter::INFO)
        // Build and init the subscriber
        .finish()
        .init();

    // prepare transactions
    let program = include_str!("../../programs/fibonacci.json").to_string();

    let transaction_type = TransactionType::FunctionExecution {
        function: "main".to_string(),
        program_name: "fibonacci.json".to_string(),
    };

    let transaction = Transaction::with_type(transaction_type).unwrap();
    info!(
        "Single benchmark transaction size: {} bytes",
        bincode::serialize(&transaction).unwrap().len()
    );

    let mut handles = vec![];

    let time = Instant::now();

    // prepare a pool of transactions for each thread in order to have them sent out as soon as possible
    for _i in 0..cli.threads {
        let mut transactions = Vec::with_capacity(cli.transactions_per_thread);

        for _i in 0..cli.transactions_per_thread {
            let t = transaction.clone();
            // in order to not have Tendermint see the transactions as duplicate and discard them,
            // clone the transactions with a different ID
            let t = Transaction {
                id: Uuid::new_v4().to_string(),
                transaction_hash: t.transaction_hash,
                transaction_type: t.transaction_type,
            };

            transactions.push(bincode::serialize(&t).unwrap());
        }
        let nodes = cli.nodes.clone();

        handles.push(tokio::spawn(async move {
            run(transactions.clone(), &nodes).await;
        }));
    }

    futures::future::join_all(handles).await;
    info!(
        "Time it took for all transactions to be delivered: {} ms",
        time.elapsed().as_millis()
    );
}

async fn run(transactions: Vec<Vec<u8>>, nodes: &Vec<SocketAddr>) {
    let time = Instant::now();
    let mut clients = vec![];
    for i in 0..nodes.len() {
        let url = format!("http://{}", &nodes.get(i).unwrap());
        clients.push(HttpClient::new(url.as_str()).unwrap());
    }

    let n_transactions = transactions.len();
    // for each transaction in this thread, send transactions in a round robin fashion to each node
    for (i, t) in transactions.into_iter().enumerate() {
        let tx: tendermint::abci::Transaction = t.into();

        let c = clients.get(i % clients.len()); // get destination node
        let response = c.unwrap().broadcast_tx_async(tx).await;

        match &response {
            Ok(_) => {}
            Err(v) => info!("failure: {}", v),
        }

        let response = response.unwrap();
        match response.code {
            tendermint::abci::Code::Ok => {}
            tendermint::abci::Code::Err(code) => {
                info!("Error executing transaction {}: {}", code, response.log);
            }
        }
    }
    info!(
        "transactions sent: {} in {} ms",
        n_transactions,
        time.elapsed().as_millis()
    );
}
