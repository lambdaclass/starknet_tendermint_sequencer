package main

import (
	"context"
	"encoding/hex"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	rollconf "github.com/rollkit/rollkit/config"
	rollconv "github.com/rollkit/rollkit/conv"
	rollnode "github.com/rollkit/rollkit/node"
	rollrpc "github.com/rollkit/rollkit/rpc"
	"github.com/spf13/viper"
	abci "github.com/tendermint/tendermint/abci/types"
	cfg "github.com/tendermint/tendermint/config"
	tmflags "github.com/tendermint/tendermint/libs/cli/flags"
	"github.com/tendermint/tendermint/libs/log"
	"github.com/tendermint/tendermint/p2p"
	"github.com/tendermint/tendermint/privval"
	"github.com/tendermint/tendermint/proxy"
	tmtypes "github.com/tendermint/tendermint/types"
)

var configFile string
var namespaceId string
var daStartHeight uint64
var aggregator bool
var daLayer string
var daConfig string
var address string
var transport string
var help bool
var fraudProofs bool
var blockTime uint64

func init() {
	flag.BoolVar(&help, "help", false, "print out available commands")
	flag.StringVar(&configFile, "config", "$HOME/.tendermint/config/config.toml", "Path to config.toml")
	flag.StringVar(&address, "address", "tcp://0.0.0.0:26658", "address of application socket")
	flag.StringVar(&transport, "transport", "socket", "either socket or grpc")
	flag.BoolVar(&fraudProofs, "fraud_proofs", false, "enable/disable fraud proofs")
	flag.Uint64Var(&blockTime, "block_time", 15, "block time for the rollup (in seconds)")
	flag.StringVar(&namespaceId, "rollkit.namespace_id", "0000000000000000", "namespace id to use")
	flag.Uint64Var(&daStartHeight, "rollkit.da_start_height", 0, "height to start at when querying blocks")
	flag.BoolVar(&aggregator, "rollkit.aggregator", true, "run node on aggregator mode or not")
	flag.StringVar(&daLayer, "rollkit.da_layer", "celestia", "data availability layer to use")
	flag.StringVar(&daConfig, "rollkit.da_config", `{"host":"127.0.0.1:18332","user":"rpcuser","pass":"rpcpass","http_post_mode":true,"disable_tls":true}`, "configuration to use for the data availability layer")
}

func main() {
	flag.Parse()
	if help {
		flag.Usage()
	} else {

		app := NewABCIRelayer(address)
		fmt.Println("daConfig: ", daConfig)

		node, server, err := newRollup(app, configFile)
		if err != nil {
			fmt.Fprintf(os.Stderr, "%v", err)
			os.Exit(1)
		}

		err = server.Start()
		if err != nil {
			fmt.Fprintf(os.Stderr, "%v", err)
			os.Exit(2)
		}

		node.Start()
		defer func() {
			node.Stop()
			// node.Wait()
		}()

		c := make(chan os.Signal, 1)
		signal.Notify(c, os.Interrupt, syscall.SIGTERM)
		<-c
		os.Exit(0)
	}
}

func newRollup(app abci.Application, configFile string) (rollnode.Node, *rollrpc.Server, error) {
	// read config
	config := cfg.DefaultConfig()
	config.RootDir = filepath.Dir(filepath.Dir(configFile))
	viper.SetConfigFile(configFile)
	if err := viper.ReadInConfig(); err != nil {
		return nil, nil, fmt.Errorf("viper failed to read config file: %w", err)
	}
	if err := viper.Unmarshal(config); err != nil {
		return nil, nil, fmt.Errorf("viper failed to unmarshal config: %w", err)
	}
	if err := config.ValidateBasic(); err != nil {
		return nil, nil, fmt.Errorf("config is invalid: %w", err)
	}

	bytes, err := hex.DecodeString(namespaceId)
	if err != nil {
		return nil, nil, err
	}

	fmt.Println("daConfig: %w", daConfig)

	// translate tendermint config to rollkit config
	nodeConfig := rollconf.NodeConfig{
		Aggregator: true,
		BlockManagerConfig: rollconf.BlockManagerConfig{
			BlockTime:     time.Duration(blockTime) * time.Second,
			FraudProofs:   fraudProofs,
			DAStartHeight: daStartHeight,
		},
		DALayer:  "bitcoin",
		DAConfig: daConfig,
	}
	copy(nodeConfig.NamespaceID[:], bytes)

	rollconv.GetNodeConfig(&nodeConfig, config)
	if err := rollconv.TranslateAddresses(&nodeConfig); err != nil {
		return nil, nil, err
	}

	// create logger
	logger := log.NewTMLogger(log.NewSyncWriter(os.Stdout))
	logger, err = tmflags.ParseLogLevel(config.LogLevel, logger, cfg.DefaultLogLevel)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to parse log level: %w", err)
	}

	// read private validator
	pv := privval.LoadFilePV(
		config.PrivValidatorKeyFile(),
		config.PrivValidatorStateFile(),
	)

	// read node key
	nodeKey, err := p2p.LoadNodeKey(config.NodeKeyFile())
	if err != nil {
		return nil, nil, fmt.Errorf("failed to load node's key: %w", err)
	}

	// keys in rollkit format
	p2pKey, err := rollconv.GetNodeKey(nodeKey)
	if err != nil {
		return nil, nil, err
	}
	signingKey, err := rollconv.GetNodeKey(&p2p.NodeKey{PrivKey: pv.Key.PrivKey})
	if err != nil {
		return nil, nil, err
	}

	genesisDoc, err := tmtypes.GenesisDocFromFile(config.GenesisFile())
	if err != nil {
		return nil, nil, err
	}

	// get ABCI client
	client, err := proxy.NewLocalClientCreator(app).NewABCIClient()
	if err != nil {
		return nil, nil, err
	}

	// create node
	node, err := rollnode.NewNode(
		context.Background(),
		nodeConfig,
		p2pKey,
		signingKey,
		client,
		genesisDoc,
		logger)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to create new rollkit node: %w", err)
	}

	server := rollrpc.NewServer(node, config.RPC, logger)

	return node, server, nil
}
