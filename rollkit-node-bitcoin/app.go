package main

import (
	abcicli "github.com/tendermint/tendermint/abci/client"
	abcitypes "github.com/tendermint/tendermint/abci/types"
)

// Implements an abcicli client as an application in order to relay
// messages to ABCI apps that are not written in Go
type ABCIRelayer struct {
	client abcicli.Client
}

var _ abcitypes.Application = (*ABCIRelayer)(nil)

func NewABCIRelayer(addr string) *ABCIRelayer {
	client := abcicli.NewSocketClient(addr, true)
	if err := client.Start(); err != nil {
		panic(err)
	}
	return &ABCIRelayer{
		client: client,
	}
}

func (app *ABCIRelayer) Info(req abcitypes.RequestInfo) abcitypes.ResponseInfo {
	res, err := app.client.InfoSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app ABCIRelayer) SetOption(req abcitypes.RequestSetOption) abcitypes.ResponseSetOption {
	return abcitypes.ResponseSetOption{}
}

func (app *ABCIRelayer) DeliverTx(req abcitypes.RequestDeliverTx) abcitypes.ResponseDeliverTx {
	res, err := app.client.DeliverTxSync(req)
	if err != nil {
		panic(err)
	}

	return *res
}

func (app *ABCIRelayer) CheckTx(req abcitypes.RequestCheckTx) abcitypes.ResponseCheckTx {
	res, err := app.client.CheckTxSync(req)
	if err != nil {
		panic(err)
	}

	return *res
}

func (app *ABCIRelayer) Commit() abcitypes.ResponseCommit {
	res, err := app.client.CommitSync()
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) Query(reqQuery abcitypes.RequestQuery) (resQuery abcitypes.ResponseQuery) {
	res, err := app.client.QuerySync(reqQuery)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) InitChain(req abcitypes.RequestInitChain) abcitypes.ResponseInitChain {
	res, err := app.client.InitChainSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) BeginBlock(req abcitypes.RequestBeginBlock) abcitypes.ResponseBeginBlock {
	// There's a serialization edge case at height 1 that will cause
	// the tower-abci server to return an error.
	// so we opt for not relaying that request
	if req.Header.Height == 1 {
		return abcitypes.ResponseBeginBlock{}
	} else {
		res, err := app.client.BeginBlockSync(req)
		if err != nil {
			panic(err)
		}
		return *res
	}
}

func (app *ABCIRelayer) EndBlock(req abcitypes.RequestEndBlock) abcitypes.ResponseEndBlock {
	res, err := app.client.EndBlockSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) ListSnapshots(req abcitypes.RequestListSnapshots) abcitypes.ResponseListSnapshots {
	res, err := app.client.ListSnapshotsSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) OfferSnapshot(req abcitypes.RequestOfferSnapshot) abcitypes.ResponseOfferSnapshot {
	res, err := app.client.OfferSnapshotSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) LoadSnapshotChunk(req abcitypes.RequestLoadSnapshotChunk) abcitypes.ResponseLoadSnapshotChunk {
	res, err := app.client.LoadSnapshotChunkSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) ApplySnapshotChunk(req abcitypes.RequestApplySnapshotChunk) abcitypes.ResponseApplySnapshotChunk {
	res, err := app.client.ApplySnapshotChunkSync(req)
	if err != nil {
		panic(err)
	}
	return *res
}

func (app *ABCIRelayer) GenerateFraudProof(abcitypes.RequestGenerateFraudProof) abcitypes.ResponseGenerateFraudProof {
	return abcitypes.ResponseGenerateFraudProof{}
}

func (ABCIRelayer) GetAppHash(abcitypes.RequestGetAppHash) abcitypes.ResponseGetAppHash {
	return abcitypes.ResponseGetAppHash{}
}

func (ABCIRelayer) VerifyFraudProof(abcitypes.RequestVerifyFraudProof) abcitypes.ResponseVerifyFraudProof {
	return abcitypes.ResponseVerifyFraudProof{}
}
