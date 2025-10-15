#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
# -a Export all variables
set -euxa
WORKDIR="/volume/packages/contracts-bedrock"
DEPLOY_CONFIG="${WORKDIR}/deploy-config/umi.json"
L1_DEPLOYMENT="${WORKDIR}/deployments/1337-deploy.json"
L2_ALLOCS="${WORKDIR}/state-dump-42069.json"
GENESIS_FILE="${WORKDIR}/deployments/genesis.json"
ROLLUP_FILE="${WORKDIR}/deployments/rollup.json"
SHARED="/volume/shared"

# Remove existing output files in the shared volume
rm -f "${SHARED}/umi.json" "${SHARED}/1337-deploy.json" "${SHARED}/state-dump-42069.json" "${SHARED}/genesis.json" "${SHARED}/rollup.json"

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

# Generate L2 genesis deploy config file
DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG}" \
./config.sh

# Deploy Optimism L1 contracts
DEPLOYMENT_CONTEXT=umi \
DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG}" \
IMPL_SALT=0000000000000000000000000000000000000000000000000000000000000000 \
forge script ${WORKDIR}/scripts/Deploy.s.sol:Deploy \
    --root "${WORKDIR}" \
    --private-key "${ADMIN_PRIVATE_KEY}" \
    --broadcast \
    --rpc-url "${L1_RPC_URL}" \
    --slow \
    --legacy \
    --non-interactive

# Generate L2 genesis state dump
CONTRACT_ADDRESSES_PATH="${L1_DEPLOYMENT}" \
DEPLOY_CONFIG_PATH="${DEPLOY_CONFIG}" \
forge script ${WORKDIR}/scripts/L2Genesis.s.sol:L2Genesis \
    --root "${WORKDIR}" \
    --sig "runWithAllUpgrades()" \
    --non-interactive

# Generate L2 genesis
op-node genesis l2 \
    --deploy-config "${DEPLOY_CONFIG}" \
    --l1-deployments "${L1_DEPLOYMENT}" \
    --l2-allocs "${L2_ALLOCS}" \
    --outfile.l2 "${GENESIS_FILE}" \
    --outfile.rollup "${ROLLUP_FILE}" \
    --l1-rpc "${L1_RPC_URL}"

# Copy output files in the shared volume
cp -f "${DEPLOY_CONFIG}" "${L2_ALLOCS}" "${L1_DEPLOYMENT}" "${GENESIS_FILE}" "${ROLLUP_FILE}" "${SHARED}/"
