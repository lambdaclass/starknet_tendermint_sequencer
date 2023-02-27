.PHONY: tendermint reset abci tendermint_config tendermint_install

OS := $(shell uname | tr '[:upper:]' '[:lower:]')

ifeq ($(shell uname -p), arm)
ARCH=arm64
else
ARCH=amd64
endif

TMINT_VERSION=0.34.22
TENDERMINT_HOME=~/.tendermint/

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

# run the Cairo tendermint application
abci:
	cargo run --release  --bin abci

# run tests on release mode (default VM backend) to ensure there is no extra printing to stdout
test:
	RUST_BACKTRACE=full cargo test --release -- --nocapture --test-threads=4

