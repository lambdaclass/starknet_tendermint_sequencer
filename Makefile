.PHONY: reset abci cli consensus_config consensus_install rollkit_celestia bitcoin celestia

OS := $(shell uname | tr '[:upper:]' '[:lower:]')

ifeq ($(shell uname -p), arm)
ARCH=arm64
else
ARCH=amd64
endif

# By default consensus protocol is tendermint. Can be overriden with cometbft
CONSENSUS=tendermint

ifeq ($(CONSENSUS), tendermint)
CONSENSUS_VERSION=0.34.22
CONSENSUS_HOME=~/.tendermint/
else
CONSENSUS=cometbft
CONSENSUS_VERSION=0.34.27
CONSENSUS_HOME=~/.cometbft/
endif

test_make:
	echo "CONSENSUS = $(CONSENSUS) version=$(CONSENSUS_VERSION) home=$(CONSENSUS_HOME)"

# Build the client program and put it in bin/aleo
cli:
	mkdir -p bin && cargo build --release && cp target/release/cli bin/cli


# Installs tendermint for current OS and puts it in bin/
bin/tendermint:
	make consensus_install CONSENSUS=tendermint

# Installs cometbft for current OS and puts it in bin/
bin/cometbft:
	make consensus_install CONSENSUS=cometbft

# Internal phony target to install tendermint/cometbft for an arbitrary OS
consensus_install:
	mkdir -p $(CONSENSUS)-install bin && cd $(CONSENSUS)-install &&\
	wget https://github.com/$(CONSENSUS)/$(CONSENSUS)/releases/download/v$(CONSENSUS_VERSION)/$(CONSENSUS)_$(CONSENSUS_VERSION)_$(OS)_$(ARCH).tar.gz &&\
	tar -xzvf $(CONSENSUS)_$(CONSENSUS_VERSION)_$(OS)_$(ARCH).tar.gz
	mv $(CONSENSUS)-install/$(CONSENSUS) bin/ && rm -rf $(CONSENSUS)-install

# Run a consensus node, installing it if necessary
node: bin/$(CONSENSUS) consensus_config
	bin/$(CONSENSUS) node

# Override a tendermint/cometbft node's default configuration. NOTE: we should do something more declarative if we need to update more settings.
consensus_config:
	sed -i.bak 's/max_body_bytes = 1000000/max_body_bytes = 12000000/g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's/max_tx_bytes = 1048576/max_tx_bytes = 10485770/g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's#laddr = "tcp://127.0.0.1:26657"#laddr = "tcp://0.0.0.0:26657"#g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's/prometheus = false/prometheus = true/g' $(CONSENSUS_HOME)/config/config.toml

# remove the blockchain data
reset: bin/$(CONSENSUS)
	bin/$(CONSENSUS) unsafe_reset_all
	rm -rf abci.height

# run the Cairo abci application
abci:
	cargo run --release  --bin abci

# run tests on release mode (default VM backend) to ensure there is no extra printing to stdout
test:
	RUST_BACKTRACE=full cargo test --release -- --nocapture --test-threads=4

# Initialize the consensus configuration for a localnet of the given amount of validators
localnet: VALIDATORS:=4
localnet: ADDRESS:=127.0.0.1
localnet: HOMEDIR:=localnet
localnet: bin/consensus cli
	rm -rf $(HOMEDIR)/
	bin/$(CONSENSUS) testnet --v $(VALIDATORS) --o ./$(HOMEDIR) --starting-ip-address $(ADDRESS)
	for n in $$(seq 0 $$(($(VALIDATORS)-1))) ; do \
        make localnet_config CONSENSUS_HOME=$(HOMEDIR)/node$$n NODE=$$n VALIDATORS=$(VALIDATORS); \
		mkdir $(HOMEDIR)/node$$n/abci ; \
	done
.PHONY: localnet
# cargo run --bin genesis --release -- $(HOMEDIR)/*

# run both the abci application and the consensus node
# assumes config for each node has been done previously
localnet_start: NODE:=0
localnet_start: HOMEDIR:=localnet
localnet_start:
	bin/$(CONSENSUS) node --home ./$(HOMEDIR)/node$(NODE) &
	cd ./$(HOMEDIR)/node$(NODE)/abci; cargo run --release --bin abci -- --port 26$(NODE)58
.PHONY: localnet_start


localnet_config:
	sed -i.bak 's/max_body_bytes = 1000000/max_body_bytes = 12000000/g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's/max_tx_bytes = 1048576/max_tx_bytes = 10485770/g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's/prometheus = false/prometheus = true/g' $(CONSENSUS_HOME)/config/config.toml
	for n in $$(seq 0 $$(($(VALIDATORS)-1))) ; do \
	    eval "sed -i.bak 's/127.0.0.$$(($${n}+1)):26656/127.0.0.1:26$${n}56/g' $(CONSENSUS_HOME)/config/config.toml" ;\
	done
	sed -i.bak 's#laddr = "tcp://0.0.0.0:26656"#laddr = "tcp://0.0.0.0:26$(NODE)56"#g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's#laddr = "tcp://127.0.0.1:26657"#laddr = "tcp://0.0.0.0:26$(NODE)57"#g' $(CONSENSUS_HOME)/config/config.toml
	sed -i.bak 's#proxy_app = "tcp://127.0.0.1:26658"#proxy_app = "tcp://127.0.0.1:26$(NODE)58"#g' $(CONSENSUS_HOME)/config/config.toml
.PHONY: localnet_config


localnet_reset:
	bin/$(CONSENSUS) unsafe_reset_all
		rm -rf localnet/node*/abci/abci.height;
.PHONY: localnet_reset

clippy:
	cargo clippy --all-targets --all-features -- -D warnings
.PHONY: clippy


celestia:
	(cd local-da; docker compose -f ./docker/test-docker-compose.yml up)

rollkit_celestia:
	(cd rollkit-node;go build)
	export NAMESPACE_ID=$$(echo $$RANDOM | md5sum | head -c 16; echo;) ;\
	./rollkit-node/rollkit-node -config "$$HOME/.tendermint/config/config.toml" -rollkit.namespace_id $$NAMESPACE_ID -rollkit.da_start_height 1

rollkit_bitcoin:
	(cd rollkit-node-bitcoin;go build)
	export NAMESPACE_ID=$$(echo $$RANDOM | md5sum | head -c 16; echo;) ;\
	./rollkit-node-bitcoin/rollkit-node-bitcoin -config "$$HOME/.tendermint/config/config.toml" -rollkit.aggregator true -rollkit.da_layer bitcoin -rollkit.da_config='{"host":"127.0.0.1:18332","user":"rpcuser","pass":"rpcpass","http_post_mode":true,"disable_tls":true}' -rollkit.namespace_id $$NAMESPACE_ID -rollkit.da_start_height 1

bitcoin:
	./bitcoin/start-daemon.sh &
	./bitcoin/run.sh 
	