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

## Reference links
* [Starknet sequencer](https://www.starknet.io/de/posts/engineering/starknets-new-sequencer#:~:text=What%20does%20the%20sequencer%20do%3F)
* [Papyrus Starknet full node](https://medium.com/starkware/papyrus-an-open-source-starknet-full-node-396f7cd90202)
* [tendermint-rs](https://github.com/informalsystems/tendermint-rs)
* [ABCI overview](https://docs.tendermint.com/v0.34/introduction/what-is-tendermint.html#abci-overview)
* [ABCI v0.34 reference](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/abci.md)
* [About why app hash is needed](https://github.com/tendermint/tendermint/issues/1179). Also [this](https://github.com/tendermint/tendermint/blob/v0.34.x/spec/abci/apps.md#query-proofs).