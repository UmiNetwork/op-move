#!/bin/sh
# Entrypoint of the op-move docker container

set -eux
SHARED="/volume/shared"
GENESIS_FILE="${SHARED}/genesis.json"
TIMEOUT_SECS=1500

# Wait for geth to become online
wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${L1_RPC_URL}" | cut -c 8-)"

# Wait for geth to deploy Optimism
while [ ! -f "${GENESIS_FILE}" ]; do sleep 1; done

/volume/op-move --genesis.l2-contract-genesis "${GENESIS_FILE}" &

# Get the PID of the geth process launched in the background
PID="$!"

# Shuts geth down in a way that does not corrupt the datadir
shutdown() {
  kill -SIGINT "${PID}"
  wait "${PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown TERM

# Block on the background geth process
wait "${PID}"
