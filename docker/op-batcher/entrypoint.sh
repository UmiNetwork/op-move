#!/bin/sh
# Entrypoint of the op-batcher docker container

set -eux
TIMEOUT_SECS=1500

wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${L1_RPC_URL}" | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${L2_RPC_URL}" | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${ROLLUP_RPC_URL}" | cut -c 8-)"

op-batcher \
  --l2-eth-rpc "${L2_RPC_URL}" \
  --rollup-rpc "${ROLLUP_RPC_URL}" \
  --poll-interval 1s \
  --sub-safety-margin 6 \
  --num-confirmations 1 \
  --safe-abort-nonce-too-low-count 3 \
  --resubmission-timeout 30s \
  --rpc.addr 0.0.0.0 \
  --rpc.port 8548 \
  --rpc.enable-admin \
  --max-channel-duration 1 \
  --private-key "${BATCHER_PRIVATE_KEY}" \
  --l1-eth-rpc "${L1_RPC_URL}" \
  --throttle.unsafe-da-bytes-lower-threshold 0 &

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
