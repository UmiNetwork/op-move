#!/bin/sh
# Entrypoint of the op-node docker container

set -eux
WORKDIR="/volume"
SHARED="${WORKDIR}/shared"
ROLLUP_FILE="${SHARED}/rollup.json"
JWT_FILE="${WORKDIR}/jwt.txt"

# Wait for op-move to serve the genesis block
while [ "$(cast block 0 --rpc-url "${L2_RPC_HTTP_URL}" >/dev/null 2>&1 ; echo $?)" -ne 0 ]; do sleep 1; done

# Wait for geth to deploy Optimism
while [ ! -f "${ROLLUP_FILE}" ]; do sleep 1; done

echo "${JWT_SECRET}" > "${JWT_FILE}"

op-node \
  --l1.beacon.ignore \
  --l2 "${L2_RPC_AUTH_URL}" \
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
  --l1 "${L1_RPC_URL}" \
  --l1.rpckind basic
