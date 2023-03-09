#!/usr/bin/env bash

sleep 5
bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass createwallet w1
bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass loadwallet w1

export COINBASE=$(bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass getnewaddress)
bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass generatetoaddress 101 $COINBASE


# Script to generate a new block every second
# Put this script at the root of your unpacked folder
#!/bin/bash

echo "Generating a block every 10 seconds. Press [CTRL+C] to stop.."

address=`bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass getnewaddress`

while :
do
        echo "Generate a new block `date '+%d/%m/%Y %H:%M:%S'`"
        bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass generatetoaddress 1 $address
        sleep 10
done


# https://rollkit.dev/docs/tutorials/bitcoin/#running-a-local-bitcoin-network
# bitcoin-cli -regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass getblockcount
# export FLAGS="-regtest -rpcport=18332 -rpcuser=rpcuser -rpcpassword=rpcpass"
# bitcoin-cli $FLAGS getblockhash 4980
# In the case that you are starting your regtest network again, you can use the following command to clear the old chain history:
# rm -rf ${LOCATION OF .bitcoin folder}








