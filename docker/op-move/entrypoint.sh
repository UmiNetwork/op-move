#!/bin/sh
# Entrypoint of the op-move docker container

set -eux
SHARED="/volume/shared"
GENESIS_FILE="${SHARED}/genesis.json"
TIMEOUT_SECS=1500

wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${L1_RPC_URL}" | cut -c 8-)"
while [ ! -f "${GENESIS_FILE}" ]; do sleep 1; done

/volume/op-move --genesis.l2-contract-genesis "${GENESIS_FILE}"
