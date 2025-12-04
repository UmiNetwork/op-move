#!/bin/sh
# Entrypoint of the op-proposer docker container

set -eux
SHARED="/volume/shared"
L1_DEPLOYMENT="${SHARED}/l1.json"
TIMEOUT_SECS=1500

wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${L1_RPC_URL} | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${ROLLUP_RPC_URL} | cut -c 8-)"

# Read the game factory address from the list of deployed contract addresses
GAME_FACTORY_ADDRESS=$(grep DisputeGameFactoryProxy "${L1_DEPLOYMENT}" | cut -d"\"" -f4)
echo "${GAME_FACTORY_ADDRESS}"

op-proposer \
  --poll-interval 12s \
  --rpc.port 8560 \
  --rollup-rpc "${ROLLUP_RPC_URL}" \
  --game-factory-address "${GAME_FACTORY_ADDRESS}" \
  --private-key "${PROPOSER_PRIVATE_KEY}" \
  --l1-eth-rpc "${L1_RPC_URL}" \
  --num-confirmations 1 \
  --game-type 1 \
  --proposal-interval 1m \
  --allow-non-finalized true &

# Get the PID of the process launched in the background
PID="$!"

# Shuts down in a way that does not corrupt the datadir
shutdown() {
  kill -SIGINT "${PID}"
  wait "${PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown TERM

# Block on the background process
wait "${PID}"
