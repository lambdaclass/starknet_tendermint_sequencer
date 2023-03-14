use eyre::Result;
use starknet_abci::transaction::{Transaction, TransactionType};

async fn send_transaction(host: &str) -> Result<()> {
    let transaction_type = TransactionType::FunctionExecution {
        function: "main".to_string(),
        program_name: "fibonacci.json".to_string(),
    };

    let tx = Transaction::with_type(transaction_type).unwrap();

    let tx = serde_json::to_string(&tx)?;

    let client = reqwest::Client::new();
    client
        .get(format!("{}/broadcast_tx", host))
        .query(&[("tx", tx)])
        .send()
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // the ABCI port on the various narwhal primaries
    let hosts = ["http://127.0.0.1:3002", "http://127.0.0.1:3009", "http://127.0.0.1:3016"];

    unsafe{
    for i in 0..200 {
        let tx_result = send_transaction(hosts.get_unchecked(i%3)).await;
        match tx_result
        {
            Ok(_) => println!("transaction committed to {}", hosts.get_unchecked(i%3)),
            Err(e) => println!("error: {}", e),
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
    Ok(())
}
