#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
# -a Export all variables
set -euxa
. /volume/.env
WORKDIR="/volume/packages/contracts-bedrock"
DEPLOY_CONFIG="${WORKDIR}/deploy-config/umi.json"
L1_DEPLOYMENT="${WORKDIR}/deployments/1337-deploy.json"
L2_ALLOCS="${WORKDIR}/state-dump-42069.json"
GENESIS_FILE="${WORKDIR}/deployments/genesis.json"
ROLLUP_FILE="${WORKDIR}/deployments/rollup.json"
SHARED="/volume/shared"

# Remove existing output files in the shared volume
rm -f "${SHARED}/umi.json" "${SHARED}/1337-deploy.json" "${SHARED}/state-dump-42069.json" "${SHARED}/genesis.json" "${SHARED}/rollup.json"

# Wait for the RPC node to become available
wait-for-it "${L1_RPC_ADDR}:${L1_RPC_PORT}"

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
