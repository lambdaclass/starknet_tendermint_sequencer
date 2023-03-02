# Tendermint-based Starknet Sequencer

## Overview

Sequencer for Starknet based in Tendermint and Cairo-rs.

## Getting started

Install Tendermint Core and store it in `/bin`
```bash
make bin/tendermint
```

Run Tendermint Core node on a terminal:

```bash
make node
```

Run the ABCI sequencer application:

```bash
make abci
```
In order to reset Tendermint's state before rerunning it, make sure you run `make reset`

### Send an execution

To send executions to the sequencer you need to have a compiled Cairo program (*.json files in the repo). Then you can send them like so:

```bash
cargo run --release programs/fibonacci.json main
```

### Benchmark

You can run a benchmark

```bash
cargo run --release --bin bench -- --nodes "{list-of-nodes}" --threads 4 --transactions-per-thread 1000
```

Where `list-of-nodes` is a list of addresses that are part of the Tendermint network (in the form of `ipaddr:socket`).
The benchmark runs `fibonacci.json` (`fib(500)`), where requests are sent with a round-robin fashion to the list of nodes, through the number of threads you specify, with the amount of transactions per thread you desire.

#### Example run

```bash
> cargo run --release --bin bench -- --nodes "127.0.0.1:26157 127.0.0.1:26057"

Time it took for all transactions to be delivered: 1308 ms
```

Note that this is the time for all transactions to return (ie; validation that they have entered the mempool), but no assumptions can be made in terms of transaction finality.
## Reference links
* [Starknet sequencer](https://www.starknet.io/de/posts/engineering/starknets-new-sequencer#:~:text=What%20does%20the%20sequencer%20do%3F)
* [Papyrus Starknet full node](https://medium.com/starkware/papyrus-an-open-source-starknet-full-node-396f7cd90202)
* [Blockifier](https://github.com/starkware-libs/blockifier)
* [tendermint-rs](https://github.com/informalsystems/tendermint-rs)
* [ABCI overview](https://docs.tendermint.com/v0.34/introduction/what-is-tendermint.html#abci-overview)
* [ABCI v0.34 reference](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/abci.md)
* [About why app hash is needed](https://github.com/tendermint/tendermint/issues/1179). Also [this](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/apps.md#query-proofs).
