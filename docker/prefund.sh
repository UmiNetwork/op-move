#!/bin/sh
# Entrypoint of the op-batcher docker container

set -eu
. /volume/.env
L1_RPC_URL="http://127.0.0.1:58138"
FROM_WALLET=$(ls l1_datadir/keystore | grep -o '.\{40\}$')
FACTORY_DEPLOYER_ADDRESS="0x3fAB184622Dc19b6109349B94811493BF2a45362"
HEARTBEAT_ADDRESS="0x88f9b82462f6c4bf4a0fb15e5c3971559a316e7f"
NONCE=0

for _ in $(seq 10) ; do
  sleep 0.1
  TX_HASH=$(curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "{\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0x${FROM_WALLET}\",\"to\":\"${ADMIN_ADDRESS}\",\"value\":\"0x8ac7230489e80000\",\"type\":\"0x2\",\"nonce\":\"$(printf  "0x%x" ${NONCE})\"}],\"id\":1,\"jsonrpc\":\"2.0\"}" | jq .result)
  echo "${TX_HASH}"
  NONCE=$((NONCE + 1))

  sleep 0.1
  curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "{\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0x${FROM_WALLET}\",\"to\":\"${BATCHER_ADDRESS}\",\"value\":\"0x8ac7230489e80000\",\"type\":\"0x2\",\"nonce\":\"$(printf  "0x%x" ${NONCE})\"}],\"id\":1,\"jsonrpc\":\"2.0\"}"
  NONCE=$((NONCE + 1))

  sleep 0.1
  curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "{\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0x${FROM_WALLET}\",\"to\":\"${PROPOSER_ADDRESS}\",\"value\":\"0x8ac7230489e80000\",\"type\":\"0x2\",\"nonce\":\"$(printf "0x%x" ${NONCE})\"}],\"id\":1,\"jsonrpc\":\"2.0\"}"
  NONCE=$((NONCE + 1))

  sleep 0.1
  curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "{\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0x${FROM_WALLET}\",\"to\":\"${FACTORY_DEPLOYER_ADDRESS}\",\"value\":\"0xde0b6b3a7640000\",\"type\":\"0x2\",\"nonce\":\"$(printf "0x%x" ${NONCE})\"}],\"id\":1,\"jsonrpc\":\"2.0\"}"
  NONCE=$((NONCE + 1))

  sleep 0.1
  TX_HASH=$(curl "${L1_RPC_URL}" \
    -s \
    -X POST \
    -H "Content-Type: application/json" \
    --data "{\"method\":\"eth_sendTransaction\",\"params\":[{\"from\":\"0x${FROM_WALLET}\",\"to\":\"${HEARTBEAT_ADDRESS}\",\"value\":\"0x8ac7230489e80000\",\"type\":\"0x2\",\"nonce\":\"$(printf "0x%x" ${NONCE})\"}],\"id\":1,\"jsonrpc\":\"2.0\"}" | jq .result)
  echo "${TX_HASH}"
  NONCE=$((NONCE + 1))
done
