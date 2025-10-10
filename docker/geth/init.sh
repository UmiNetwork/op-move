#!/bin/bash
# Background tasks of the geth docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
# -a Export all variables
set -euxa
. /volume/.env
L1_RPC_URL="http://0.0.0.0:${L1_RPC_PORT}"

# Wait for the RPC node to become available
wait-for-it "${L1_RPC_ADDR}:${L1_RPC_PORT}"
