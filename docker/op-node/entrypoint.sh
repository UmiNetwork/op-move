#!/bin/sh
# Entrypoint of the op-node docker container

set -eux
WORKDIR="/volume"
SHARED="${WORKDIR}/shared"
ROLLUP_FILE="${SHARED}/rollup.json"
L1_GENESIS_FILE="${SHARED}/l1_genesis.json"
JWT_FILE="${WORKDIR}/jwt.txt"
P2P_DIR="${WORKDIR}/host/p2p"

# Dump JWT secret token to a file that gets passed as op-node CLI argument
echo "${JWT_SECRET}" >"${JWT_FILE}"

# Make dir for persistent P2P data
mkdir -p ${P2P_DIR}

# Wait for op-move to serve the genesis block
while [ "$(
  cast block 0 --rpc-url "${L2_RPC_HTTP_URL}" >/dev/null 2>&1
  echo $?
)" -ne 0 ]; do sleep 1; done

# Wait for geth to deploy Optimism
while [ ! -f "${ROLLUP_FILE}" ]; do sleep 1; done

op-node \
  --l1 "${L1_RPC_URL}" \
  --l1.beacon.ignore \
  --l1.rpckind basic \
  --l2 "${L2_RPC_AUTH_URL}" \
  --l2.jwt-secret "${JWT_FILE}" \
  --sequencer.enabled \
  --sequencer.l1-confs 5 \
  --verifier.l1-confs 4 \
  --rollup.config "${ROLLUP_FILE}" \
  --rpc.addr 0.0.0.0 \
  --rpc.port "${RPC_PORT}" \
  --rpc.enable-admin \
  --p2p.listen.ip 0.0.0.0 \
  --p2p.listen.tcp "${P2P_PORT}" \
  --p2p.listen.udp "${P2P_PORT}" \
  --p2p.sequencer.key "${SEQUENCER_PRIVATE_KEY}" \
  --p2p.priv.path "${P2P_DIR}/priv.txt" \
  --p2p.peerstore.path "${P2P_DIR}/peerstore_db" \
  --p2p.discovery.path "${P2P_DIR}/discovery_db" \
  --rollup.l1-chain-config "${L1_GENESIS_FILE}" &

# Get the PID of the op-node process launched in the background
PID="$!"

# Shuts op-node down in a way that does not corrupt the datadir
shutdown() {
  kill -SIGINT "${PID}"
  wait "${PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown TERM

# Block on the background op-node process
wait "${PID}"
