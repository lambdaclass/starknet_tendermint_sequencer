package abci

import (
    "github.com/informalsystems/tm-load-test/pkg/loadtest"
    "github.com/google/uuid"
    "log"
    "os/exec"
    "bytes"
)

// MyABCIAppClientFactory creates instances of MyABCIAppClient
type MyABCIAppClientFactory struct {}

// MyABCIAppClientFactory implements loadtest.ClientFactory
var _ loadtest.ClientFactory = (*MyABCIAppClientFactory)(nil)

// MyABCIAppClient is responsible for generating transactions. Only one client
// will be created per connection to the remote Tendermint RPC endpoint, and
// each client will be responsible for maintaining its own state in a
// thread-safe manner.
type MyABCIAppClient struct {
    tx []byte
}

// MyABCIAppClient implements loadtest.Client
var _ loadtest.Client = (*MyABCIAppClient)(nil)

func (f *MyABCIAppClientFactory) ValidateConfig(cfg loadtest.Config) error {
    // Do any checks here that you need to ensure that the load test 
    // configuration is compatible with your client.
    return nil
}

func (f *MyABCIAppClientFactory) NewClient(cfg loadtest.Config) (loadtest.Client, error) {
    cmd := exec.Command("cargo", "run",  "--release", "programs/fibonacci.json", "main", "--no-broadcast")
    var outb, errb bytes.Buffer
    cmd.Stdout = &outb
    cmd.Stderr = &errb
    err := cmd.Run()
    if err != nil {
        log.Fatal(err)
    }
    return &MyABCIAppClient{tx: outb.Bytes()}, nil
}

// GenerateTx must return the raw bytes that make up the transaction for your
// ABCI app. The conversion to base64 will automatically be handled by the 
// loadtest package, so don't worry about that. Only return an error here if you
// want to completely fail the entire load test operation.
func (c *MyABCIAppClient) GenerateTx() ([]byte, error) {
    var newTx []byte
    newTx = append(newTx, c.tx[0:8]...)
    // Replacing the uuid with a new random one to prevent getting duplicated tx rejected
    newTx = append(newTx, []byte(uuid.New().String())...)
    newTx = append(newTx, c.tx[44:]...)
    return newTx, nil
}