#!/bin/bash
# Entrypoint of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
set -eux
L1_DATADIR="./l1_datadir"
INIT_OP="./init-op.sh"

# Deploy Optimism on the L1 and produce deploy artifacts for L2
if [ -f "${INIT_OP}" ]; then ${INIT_OP} ; rm -f ${INIT_OP}; fi

# Ephemeral proof-of-authority network with a pre-funded developer account,
# with automatic mining when there are pending transactions.
geth \
  --dev \
  --dev.period "${L1_BLOCK_TIME}" \
  --datadir "${L1_DATADIR}" \
  --rpc.allow-unprotected-txs \
  --http \
  --http.addr 0.0.0.0 \
  --http.port 58138 \
  --http.corsdomain '*' \
  --http.api 'web3,debug,eth,txpool,net,engine' \
  --http.vhosts '*' &

# Get the PID of the geth process launched in the background
GETH_PID="$!"

# Shuts geth down in a way that does not corrupt the datadir
function shutdown() {
  kill -SIGINT "${GETH_PID}"
  wait "${GETH_PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown SIGTERM

# Block on the background geth process
wait "${GETH_PID}"
