#!/bin/sh
# Entrypoint of the op-proposer docker container

set -eux
. /volume/.env
SHARED="/volume/shared"
L1_RPC_URL="http://geth:58138"
ROLLUP_RPC_URL="http://op-node:8547"
L1_DEPLOYMENT="${SHARED}/1337-deploy.json"
TIMEOUT_SECS=300

wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${L1_RPC_URL} | cut -c 8-)"
wait-for-it -t "${TIMEOUT_SECS}" "$(echo ${ROLLUP_RPC_URL} | cut -c 8-)"

# Read the oracle address from the list of deployed contract addresses
L2_ORACLE_PROXY=$(jq -r .L2OutputOracleProxy "${L1_DEPLOYMENT}")

op-proposer \
    --poll-interval 12s \
    --rpc.port 8560 \
    --rollup-rpc "${ROLLUP_RPC_URL}" \
    --l2oo-address "${L2_ORACLE_PROXY}" \
    --private-key "${PROPOSER_PRIVATE_KEY}" \
    --l1-eth-rpc "${L1_RPC_URL}" \
    --num-confirmations 1 \
    --allow-non-finalized true
