use anyhow::{bail, ensure, Result};
use lib::Transaction;
use tendermint_rpc::{query::Query, Client, HttpClient, Order};
use tracing::debug;

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

pub async fn get_transaction(tx_id: &str, url: &str) -> Result<Transaction> {
    let client = HttpClient::new(url)?;
    // todo: this index key might have to be a part of the shared lib so that both the CLI and the ABCI can be in sync
    let query = Query::contains("app.tx_id", tx_id);

    let response = client
        .tx_search(query, false, 1, 1, Order::Ascending)
        .await?;

    // early return with error if no transaction has been indexed for that tx id
    ensure!(
        response.total_count > 0,
        "Transaction ID {} is invalid or has not yet been committed to the blockchain",
        tx_id
    );

    let tx_bytes = response.txs.into_iter().next().unwrap().tx;
    let transaction: Transaction = bincode::deserialize(&tx_bytes)?;

    Ok(transaction)
}
