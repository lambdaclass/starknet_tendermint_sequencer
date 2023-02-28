use clap::Parser;
use lib::{Transaction, TransactionType};
use tracing::{info, trace};
use std::time::Instant;
use std::{net::SocketAddr};
use tendermint_rpc::{Client, HttpClient};
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
    pub transactions_per_second: usize,

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
        // Build and init the subscriber
        .finish()
        .init();

    // prepare transactions
    let program = include_str!("../../programs/fibonacci.json").to_string();

    let transaction_type = TransactionType::FunctionExecution {
        program,
        function: "main".to_string(),
        program_name: "fibonacci".to_string(),
    };
    let transaction = Transaction::with_type(transaction_type).unwrap();

    let mut handles = vec![];

    let time = Instant::now();

    for _i in 0..cli.threads {
        let mut transactions: Vec<Transaction> = Vec::with_capacity(cli.transactions_per_second);

        for _i in 0..cli.transactions_per_second {
            let t = transaction.clone();

            transactions.push(Transaction {
                id: Uuid::new_v4().to_string(),
                transaction_hash: t.transaction_hash,
                transaction_type: t.transaction_type,
            });
        }
        let nodes =  cli.nodes.clone();


        handles.push(tokio::spawn(async move {
            run(transactions.clone(), &nodes).await;
        }));

    }

    futures::future::join_all(handles).await;
    info!("Time it took for all transactions to be delivered: {}", time.elapsed().as_millis());

}

async fn run(transactions: Vec<Transaction>, nodes: &Vec<SocketAddr>) {
    let time = Instant::now();
    let mut clients = vec![];
    for i in 0..nodes.len() {
        let mut url = "http://".to_owned();
        url.push_str(&nodes.get(i).unwrap().to_string());
        clients.push(HttpClient::new(url.as_str()).unwrap());
    }

    for (i, t) in transactions.into_iter().enumerate() {
        let transaction_bin = bincode::serialize(&t);

        let tx: tendermint::abci::Transaction = transaction_bin.unwrap().into();

        let c = clients.get(i % clients.len());
        let response = c.unwrap().broadcast_tx_sync(tx).await;

        match &response {
            Ok(_) => {},
            Err(v) => info!("failure: {}", v),
        }

        //let response = response.unwrap();
        //match response.code {
        //    tendermint::abci::Code::Ok => {},
        //    tendermint::abci::Code::Err(code) => {
        //        info!("Error executing transaction {}: {}", code, response.log);
        //    }
        //}
    }
    trace!("time doing transactions: {}", time.elapsed().as_millis());
}
