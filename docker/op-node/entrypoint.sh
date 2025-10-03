#!/bin/sh
# Entrypoint of the op-node docker container

# -o allexport Export all defined variables for use in config.sh
set -euxo allexport
. /volume/.env
WORKDIR="/volume/packages/contracts-bedrock"
SHARED="/volume/shared"
ROLLUP_FILE="${WORKDIR}/deployments/rollup.json"
JWT_FILE="${WORKDIR}/deployments/jwt.txt"
GENESIS_FILE="${WORKDIR}/deployments/genesis.json"
L1_DEPLOYMENT="${WORKDIR}/deployments/1337-deploy.json"
L1_RPC_URL="http://geth:58138"
L2_RPC_URL="http://op-move:8551"
OP_MOVE_ADDR="op-move"
OP_MOVE_PORT="8545"

echo "${JWT_SECRET}" > "${JWT_FILE}"
cp -f "${L1_DEPLOYMENT}" "${SHARED}/1337-deploy.json"
cp -f "${GENESIS_FILE}" "${SHARED}/genesis.json"

# Wait for op-move to serve the genesis block
while [ "$(cast block 0 --rpc-url http://${OP_MOVE_ADDR}:${OP_MOVE_PORT} >/dev/null 2>&1 ; echo $?)" -ne 0 ]; do sleep 1; done

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
