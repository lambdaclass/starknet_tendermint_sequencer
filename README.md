_Note: `main` branch contains the development version of the sequencer, for rollups integration please check branch [`rollups`](https://github.com/lambdaclass/starknet_tendermint_sequencer/tree/rollups)_
# Tendermint-based Starknet sequencer

Sequencer for Starknet based in CometBFT (formerly Tendermint Core) and [starknet-in-rust](https://github.com/lambdaclass/starknet_in_rust).

## Getting started

First, install CometBFT:

````
make consensus_install
````
This will install CometBFT and leave the binary in `bin/comebft`. Additionally, it will run `cometbft init` which creates the required configuration file and stores it in `~/.cometbft/`.

You can now run the sequencer ABCI with CometBFT by just running those two binaries.

Run CometBFT node on a terminal:

```bash
make node
```
Run the ABCI sequencer application:

```bash
make abci
```
In order to reset CometBFT's state before rerunning it, make sure you run `make reset`

### Sending a transaction

To send executions to the sequencer you need to have a compiled Cairo program (*.json files in the repo). Then you can send them like so:

```bash
cargo run --release execute sequencer/programs/fibonacci.json main
```

### Running Tendermint Core instead of CometBFT

Current code can be run with both Tendermint and CometBFT (up to version 0.34.27). In order to use Tendermint Core the make command should include the `CONSENSUS` variable:

```bash
make node CONSENSUS=tendermint
```

This will run Tendermint Core instead of CometBFT (and also will install and configure it if not present).

### Benchmark

You can run a benchmark with

```bash
cd sequencer
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

### Benchmarking with Tendermint Load Testing Framework

There is an alternate way to benchmark the app: using [tm-load-test](https://github.com/informalsystems/tm-load-test). In order to do that there is a load_test command written in go in `/bench` directory. This needs Go v1.12 at least to be built.

To build it:

```bash
cd bench
go build -o ./build/load_test ./cmd/load_test/main.go
```

and once built move back to root directory and use

```bash
./bench/build/load_test -c 10 -T 10 -r 1000 -s 250 --broadcast-tx-method async --endpoints ws://localhost:26657/websocket --stats-output result.csv
```

to run it against a local tendermint+abci node.

`-c` is the amount of connectios per endpoint

`-T` is the amount of seconds to run the test

`-r` is the rate of tx per second to send

`-s` is the maximum size of a transaction to be sent.

Run
```bash
./bench/build/load_test -h
```
to get further information.

In result.csv there will be a summary of the operation. eg:
```bash
> cat result.csv
Parameter,Value,Units
total_time,10.875,seconds
total_txs,55249,count
total_bytes,1242747923,bytes
avg_tx_rate,5080.169436,transactions per second
avg_data_rate,114271208.808144,bytes per second
```

To run it against a cluster, several nodes can be provided in `--endpoints` parameter. eg:
```bash
./bench/build/load_test -c 5 -T 10 -r 1000 -s 250 --broadcast-tx-method async --endpoints ws://5.9.57.45:26657/websocket,ws://5.9.57.44:26657/websocket,ws://5.9.57.89:26657/websocket --stats-output result.csv
```

Check [tm-load-test](https://github.com/informalsystems/tm-load-test) and [Tendermint Load Testing Framework](https://github.com/informalsystems/tm-load-test/tree/main/pkg/loadtest) and for more information.

## Reference links
* [Starknet sequencer](https://www.starknet.io/de/posts/engineering/starknets-new-sequencer#:~:text=What%20does%20the%20sequencer%20do%3F)
* [Papyrus Starknet full node](https://medium.com/starkware/papyrus-an-open-source-starknet-full-node-396f7cd90202)
* [Blockifier](https://github.com/starkware-libs/blockifier)
* [tendermint-rs](https://github.com/informalsystems/tendermint-rs)
* [ABCI overview](https://docs.tendermint.com/v0.34/introduction/what-is-tendermint.html#abci-overview)
* [ABCI v0.34 reference](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/abci.md)
* [CometBFT](https://github.com/cometbft/cometbft)
* [About why app hash is needed](https://github.com/tendermint/tendermint/issues/1179). Also [this](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/apps.md#query-proofs).
* [About Tendermint 0.34's future](https://github.com/tendermint/tendermint/issues/9972)
### Starknet
* [Starknet State](https://docs.starknet.io/documentation/architecture_and_concepts/State/starknet-state/)
* [Starknet architecture](https://david-barreto.com/starknets-architecture-review/)
* [Starknet transaction lifecylce](https://docs.starknet.io/documentation/architecture_and_concepts/Blocks/transaction-life-cycle/)
