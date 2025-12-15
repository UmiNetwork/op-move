#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
# -a Export all variables
set -euxa
WORKDIR="/volume"
GENESIS_FILE="${WORKDIR}/genesis.json"
ROLLUP_FILE="${WORKDIR}/rollup.json"
L1_GENESIS_FILE="${WORKDIR}/l1_genesis.json"
L1_DEPLOYMENT="${WORKDIR}/l1.json"
SHARED="/volume/shared"

# Remove existing output files in the shared volume
rm -f "${SHARED}/l1.json" "${SHARED}/genesis.json" "${SHARED}/rollup.json" "${SHARED}/l1_genesis.json"

# Create datadir for geth
mkdir -p "${L1_DATADIR}"

# Initialize keystore in the datadir
./keystore.sh

# Wait for the RPC node to become available
wait-for-it "${L1_RPC_ADDR}:${L1_RPC_PORT}"

# Prefund Optimism service accounts
./prefund.sh

# Deploy Optimism factory deployer contract
cast publish --rpc-url "${L1_RPC_URL}" "${SIGNED_L1_CONTRACT_TX}"

# Wait for a finalized block with a positive timestamp
while [ "$(cast block finalized --rpc-url "${L1_RPC_URL}" | awk '/^timestamp/ { print $2 }')" -le 0 ]; do sleep 3; done

# Generate L1 genesis (trying it directly on datadir throws an error)
cp -r "${L1_DATADIR}" /tmp/geth-copy
geth dumpgenesis --datadir /tmp/geth-copy >"${L1_GENESIS_FILE}"

# Deploy Optimism L1 contracts
op-deployer init --l2-chain-ids=42069 --l1-chain-id=1337 --intent-type custom

cp filled_intent.toml ./intent.toml

op-deployer bootstrap superchain \
  --l1-rpc-url="${L1_RPC_URL}" \
  --private-key="${ADMIN_PRIVATE_KEY}" \
  --superchain-proxy-admin-owner "${ADMIN_ADDRESS}" \
  --protocol-versions-owner "${ADMIN_ADDRESS}" \
  --guardian "${ADMIN_ADDRESS}" \
  --outfile superchain-output.json

SUPERCHAIN_PROXY_ADMIN=$(jq -r '.proxyAdminAddress' superchain-output.json)
SUPERCHAIN_CONFIG_PROXY=$(jq -r '.superchainConfigProxyAddress' superchain-output.json)
PROTOCOL_VERSIONS_PROXY=$(jq -r '.protocolVersionsProxyAddress' superchain-output.json)

op-deployer bootstrap implementations \
  --l1-rpc-url="${L1_RPC_URL}" \
  --private-key="${ADMIN_PRIVATE_KEY}" \
  --upgrade-controller "${ADMIN_ADDRESS}" \
  --challenger "${ADMIN_ADDRESS}" \
  --superchain-config-proxy "${SUPERCHAIN_CONFIG_PROXY}" \
  --protocol-versions-proxy "${PROTOCOL_VERSIONS_PROXY}" \
  --superchain-proxy-admin "${SUPERCHAIN_PROXY_ADMIN}"

# Fill L2 genesis state dump
op-deployer apply \
  --l1-rpc-url="${L1_RPC_URL}" \
  --private-key="${ADMIN_PRIVATE_KEY}"

# Generate L2 genesis file
op-deployer inspect genesis 42069 >genesis.json

# Generate rollup file
op-deployer inspect rollup 42069 >rollup.json

# TODO: DGF address
op-deployer inspect l1 42069 >l1.json

# Copy output files in the shared volume
cp -f "${L1_DEPLOYMENT}" "${GENESIS_FILE}" "${ROLLUP_FILE}" "${L1_GENESIS_FILE}" "${SHARED}/"
