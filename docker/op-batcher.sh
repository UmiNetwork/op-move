#!/bin/sh
# Entrypoint of the op-batcher docker container

set -eux
. /volume/.env
L1_RPC_URL="http://geth:58138"
L2_RPC_URL="http://op-move:8545"
ROLLUP_RPC_URL="http://op-node:8547"
TIMEOUT_SECS=1500

wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${L1_RPC_URL} | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${L2_RPC_URL} | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${ROLLUP_RPC_URL} | cut -c 8-)"

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
    --l1-eth-rpc ${L1_RPC_URL}
