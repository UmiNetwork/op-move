#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
# -a Export all variables
set -euxa

# Ephemeral proof-of-authority network with a pre-funded developer account,
# with automatic mining when there are pending transactions.
geth \
  --dev \
  --datadir "${L1_DATADIR}" \
  --rpc.allow-unprotected-txs \
  --http \
  --http.addr 127.0.0.1 \
  --http.port 58138 \
  --http.corsdomain '*' \
  --http.api 'web3,debug,eth,txpool,net,engine' \
  --http.vhosts '*' &

# Get the PID of the geth process launched in the background
GETH_PID="$!"

# Deploy Optimism on the L1 and produce deploy artifacts for L2
./deploy-optimism.sh

# Gracefully shut geth down by sending SIGINT to avoid breaking datadir
kill -SIGINT "${GETH_PID}"

# Wait for geth to shutdown
wait "${GETH_PID}"
