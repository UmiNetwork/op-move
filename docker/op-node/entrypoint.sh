#!/bin/sh
# Entrypoint of the op-node docker container

set -eux
. /volume/.env
WORKDIR="/volume"
SHARED="/volume/shared"
ROLLUP_FILE="${SHARED}/rollup.json"
JWT_FILE="${WORKDIR}/jwt.txt"
L1_RPC_URL="http://geth:58138"
OP_MOVE_ADDR="op-move"
OP_MOVE_PORT="8545"
L2_RPC_URL="http://${OP_MOVE_ADDR}:8551"

# Wait for op-move to serve the genesis block
while [ "$(cast block 0 --rpc-url http://${OP_MOVE_ADDR}:${OP_MOVE_PORT} >/dev/null 2>&1 ; echo $?)" -ne 0 ]; do sleep 1; done

# Wait for geth to deploy Optimism
while [ ! -f "${ROLLUP_FILE}" ]; do sleep 1; done

echo "${JWT_SECRET}" > "${JWT_FILE}"

op-node \
  --l1.beacon.ignore \
  --l2 "${L2_RPC_URL}" \
  --l2.jwt-secret "${JWT_FILE}" \
  --sequencer.enabled \
  --sequencer.l1-confs 5 \
  --verifier.l1-confs 4 \
  --rollup.config "${ROLLUP_FILE}" \
  --rpc.addr 0.0.0.0 \
  --rpc.port 8547 \
  --p2p.disable \
  --rpc.enable-admin \
  --p2p.sequencer.key "${SEQUENCER_PRIVATE_KEY}" \
  --l1 ${L1_RPC_URL} \
  --l1.rpckind basic
