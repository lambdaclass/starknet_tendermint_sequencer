package main

import (
    "github.com/informalsystems/tm-load-test/pkg/loadtest"
    "github.com/lambdaclass/load_tester/pkg/abci"
)

func main() {
    if err := loadtest.RegisterClientFactory("my-abci-app-name", &abci.MyABCIAppClientFactory{}); err != nil {
        panic(err)
    }
    // The loadtest.Run method will handle CLI argument parsing, errors, 
    // configuration, instantiating the load test and/or coordinator/worker
    // operations, etc. All it needs is to know which client factory to use for
    // its load testing.
    loadtest.Run(&loadtest.CLIConfig{
        AppName:              "my-load-tester",
        AppShortDesc:         "Load testing application for My ABCI App (TM)",
        AppLongDesc:          "Some long description on how to use the tool",
        DefaultClientFactory: "my-abci-app-name",
    })
}