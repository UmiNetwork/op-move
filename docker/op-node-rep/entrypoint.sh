#!/bin/sh
# Entrypoint of the op-node-rep docker container

set -eux
WORKDIR="/volume"
SHARED="${WORKDIR}/shared"
P2P_DIR="${WORKDIR}/host/p2p"
ROLLUP_FILE="${SHARED}/rollup.json"
JWT_FILE="${WORKDIR}/jwt.txt"
TIMEOUT_SECS=1500
PEER_RPC="http://${PEER_ADDR}:${PEER_RPC_PORT}"

# Dump JWT secret token to a file that gets passed as op-node CLI argument
echo "${JWT_SECRET}" > "${JWT_FILE}"

# Make dir for persistent P2P data
mkdir -p ${P2P_DIR}

# Wait for op-move to serve the genesis block
while [ "$(cast block 0 --rpc-url "${L2_RPC_HTTP_URL}" >/dev/null 2>&1 ; echo $?)" -ne 0 ]; do sleep 1; done

# Wait for geth to deploy Optimism
while [ ! -f "${ROLLUP_FILE}" ]; do sleep 1; done

# Wait for peer to become online
wait-for-it -t "${TIMEOUT_SECS}" "${PEER_ADDR}:${PEER_RPC_PORT}"

# Parse peer P2P parameters, wait for PEER_ENR to contain IP (test by length)
while
  PEER_P2P=$(curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"opp2p_self","params":[],"id":1}' "${PEER_RPC}");
  PEER_ENR=$(echo "${PEER_P2P}" | sed -e 's/.*\"ENR\"\:\"//g' | sed -e 's/\".*//g');
  [ ${#PEER_ENR} -lt 220 ];
do sleep 5; done

# Set peer manually
PEER_ID=$(echo "${PEER_P2P}" | sed -e 's/.*\"peerID\"\:\"//g' | sed -e 's/\".*//g');
PEER_IP=$(nc -vz "${PEER_ADDR}" "${PEER_P2P_PORT}" 2>&1 | cut -d '(' -f2 | cut -d ':' -f1)
P2P_STATIC="/ip4/${PEER_IP}/tcp/${PEER_P2P_PORT}/p2p/${PEER_ID}"
P2P_BOOTNODES="${PEER_ENR}"

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
  --p2p.bootnodes "${P2P_BOOTNODES}" \
  --p2p.sequencer.key "${SEQUENCER_PRIVATE_KEY}" \
  --p2p.static "${P2P_STATIC}" \
  --p2p.priv.path "${P2P_DIR}/priv.txt" \
  --p2p.peerstore.path "${P2P_DIR}/peerstore_db" \
  --p2p.discovery.path "${P2P_DIR}/discovery_db"  &

# Get the PID of the process launched in the background
PID="$!"

# Passes SIGINT to the process and waits for it to end on its own
shutdown() {
  kill -SIGINT "${PID}"
  wait "${PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown TERM

# Block on the background process
wait "${PID}"
