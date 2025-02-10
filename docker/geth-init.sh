#!/bin/bash
# Entrypoint of the op-batcher docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
set -eux
. /volume/.env
L1_RPC_URL="http://127.0.0.1:58138"
SIGNED_L1_CONTRACT_TX="0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222"

# Wait for the RPC node to become available
wait-for-it "${L1_RPC_ADDR}:${L1_RPC_PORT}"

# Prefund Optimism service accounts
./prefund.sh

# Deploy Optimism factory deployer contract
cast publish --async --rpc-url "${L1_RPC_URL}" "${SIGNED_L1_CONTRACT_TX}"
