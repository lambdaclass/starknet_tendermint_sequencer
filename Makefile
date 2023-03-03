.PHONY: tendermint reset abci cli tendermint_config tendermint_install

OS := $(shell uname | tr '[:upper:]' '[:lower:]')

ifeq ($(shell uname -p), arm)
ARCH=arm64
else
ARCH=amd64
endif

TMINT_VERSION=0.34.22
TENDERMINT_HOME=~/.tendermint/

# Build the client program and put it in bin/aleo
cli:
	mkdir -p bin && cargo build --release && cp target/release/cli bin/cli


# Installs tendermint for current OS and puts it in bin/
bin/tendermint:
	make tendermint_install
	mv tendermint-install/tendermint bin/ && rm -rf tendermint-install

# Internal phony target to install tendermint for an arbitrary OS
tendermint_install:
	mkdir -p tendermint-install bin && cd tendermint-install &&\
	wget https://github.com/tendermint/tendermint/releases/download/v$(TMINT_VERSION)/tendermint_$(TMINT_VERSION)_$(OS)_$(ARCH).tar.gz &&\
	tar -xzvf tendermint_$(TMINT_VERSION)_$(OS)_$(ARCH).tar.gz

# Run a tendermint node, installing it if necessary
node: tendermint_config
	bin/tendermint node --consensus.create_empty_blocks_interval="30s"

# Override a tendermint node's default configuration. NOTE: we should do something more declarative if we need to update more settings.
tendermint_config:
	sed -i.bak 's/max_body_bytes = 1000000/max_body_bytes = 12000000/g' $(TENDERMINT_HOME)/config/config.toml
	sed -i.bak 's/max_tx_bytes = 1048576/max_tx_bytes = 10485770/g' $(TENDERMINT_HOME)/config/config.toml
	sed -i.bak 's#laddr = "tcp://127.0.0.1:26657"#laddr = "tcp://0.0.0.0:26657"#g' $(TENDERMINT_HOME)/config/config.toml

# remove the blockchain data
reset: bin/tendermint
	bin/tendermint unsafe_reset_all
	rm -rf abci.height

# run the Cairo tendermint application
abci:
	cargo run --release  --bin abci

# run tests on release mode (default VM backend) to ensure there is no extra printing to stdout
test:
	RUST_BACKTRACE=full cargo test --release -- --nocapture --test-threads=4


# Initialize the tendermint configuration for a localnet of the given amount of validators
localnet: VALIDATORS:=4
localnet: ADDRESS:=127.0.0.1
localnet: HOMEDIR:=localnet
localnet: bin/tendermint cli
	rm -rf $(HOMEDIR)/
	bin/tendermint testnet --v $(VALIDATORS) --o ./$(HOMEDIR) --starting-ip-address $(ADDRESS)
	for n in $$(seq 0 $$(($(VALIDATORS)-1))) ; do \
        make localnet_config TENDERMINT_HOME=$(HOMEDIR)/node$$n NODE=$$n VALIDATORS=$(VALIDATORS); \
		mkdir $(HOMEDIR)/node$$n/abci ; \
	done
.PHONY: localnet
# cargo run --bin genesis --release -- $(HOMEDIR)/*

# run both the abci application and the tendermint node
# assumes config for each node has been done previously
localnet_start: NODE:=0
localnet_start: HOMEDIR:=localnet
localnet_start:
	bin/tendermint node --home ./$(HOMEDIR)/node$(NODE) --consensus.create_empty_blocks_interval="90s" &
	cd ./$(HOMEDIR)/node$(NODE)/abci; cargo run --release --bin abci -- --port 26$(NODE)58
.PHONY: localnet_start


localnet_config:
	sed -i.bak 's/max_body_bytes = 1000000/max_body_bytes = 12000000/g' $(TENDERMINT_HOME)/config/config.toml
	sed -i.bak 's/max_tx_bytes = 1048576/max_tx_bytes = 10485770/g' $(TENDERMINT_HOME)/config/config.toml
	for n in $$(seq 0 $$(($(VALIDATORS)-1))) ; do \
	    eval "sed -i.bak 's/127.0.0.$$(($${n}+1)):26656/127.0.0.1:26$${n}56/g' $(TENDERMINT_HOME)/config/config.toml" ;\
	done
	sed -i.bak 's#laddr = "tcp://0.0.0.0:26656"#laddr = "tcp://0.0.0.0:26$(NODE)56"#g' $(TENDERMINT_HOME)/config/config.toml
	sed -i.bak 's#laddr = "tcp://127.0.0.1:26657"#laddr = "tcp://0.0.0.0:26$(NODE)57"#g' $(TENDERMINT_HOME)/config/config.toml
	sed -i.bak 's#proxy_app = "tcp://127.0.0.1:26658"#proxy_app = "tcp://127.0.0.1:26$(NODE)58"#g' $(TENDERMINT_HOME)/config/config.toml
.PHONY: localnet_config


localnet_reset:
	bin/tendermint unsafe_reset_all
		rm -rf localnet/node*/abci/abci.height;
.PHONY: localnet_reset

.PHONY: clippy
clippy:
	cargo clippy --all-targets --all-features -- -D warnings
