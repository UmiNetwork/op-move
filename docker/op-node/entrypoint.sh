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
  --rpc.port 8547 \
  --rpc.enable-admin \
  --p2p.listen.ip 0.0.0.0 \
  --p2p.listen.tcp 9003 \
  --p2p.listen.udp 9003 \
  --p2p.bootnodes "${P2P_BOOTNODES}" \
  --p2p.sequencer.key "${SEQUENCER_PRIVATE_KEY}" \
  --p2p.static "${P2P_STATIC}" \
  --p2p.priv.path "${SHARED}/${P2P_ID}/opnode_p2p_priv.txt" \
  --p2p.peerstore.path "${SHARED}/${P2P_ID}/opnode_peerstore_db" \
  --p2p.discovery.path "${SHARED}/${P2P_ID}/opnode_discovery_db" \
  --syncmode "${SYNCMODE}" &

# Get the PID of the geth process launched in the background
PID="$!"

# Shuts geth down in a way that does not corrupt the datadir
shutdown() {
  kill -SIGINT "${PID}"
  wait "${PID}"
  exit 0
}

# Trap signal from docker stop to the graceful shutdown function
trap shutdown TERM

# Block on the background geth process
wait "${PID}"
