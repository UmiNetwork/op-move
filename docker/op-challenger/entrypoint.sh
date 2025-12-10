#!/bin/sh
# Lightweight challenger loop to mirror the integration test behaviour.
# Resolves claims for newly created games and finalizes them once ready.

set -eux

SHARED="/volume/shared"
L1_DEPLOYMENT="${SHARED}/l1.json"
RESOLVE_INTERVAL="${RESOLVE_INTERVAL:-30}"
CLAIM_RETRIES="${CLAIM_RETRIES:-6}"
CLAIM_RETRY_DELAY="${CLAIM_RETRY_DELAY:-10}"
TIMEOUT_SECS=1500

# Wait until geth is accepting connections
wait-for-it -t "${TIMEOUT_SECS}" "$(echo "${L1_RPC_URL}" | cut -c 8-)"

# DisputeGameFactoryProxy lives in the L1 deployment manifest produced by op-deployer
GAME_FACTORY_ADDRESS="$(jq -r '.DisputeGameFactoryProxy' "${L1_DEPLOYMENT}")"
if [ -z "${GAME_FACTORY_ADDRESS}" ] || [ "${GAME_FACTORY_ADDRESS}" = "null" ]; then
  echo "Unable to read DisputeGameFactoryProxy from ${L1_DEPLOYMENT}" >&2
  exit 1
fi

game_count() {
  cast call "${GAME_FACTORY_ADDRESS}" "gameCount()(uint256)" --rpc-url "${L1_RPC_URL}" | tail -n1 | awk 'END{print $NF}' || return 1
}

game_address() {
  addr="$(cast call "${GAME_FACTORY_ADDRESS}" "gameAtIndex(uint256)(uint32,uint64,address)" "$1" --rpc-url "${L1_RPC_URL}" | tail -n1 | awk '{print $NF}' || true)"
  if [ -z "${addr}" ] || [ "${addr}" = "null" ]; then
    echo "Failed to read game address at index $1" >&2
    return 1
  fi
  echo "${addr}"
}

echo "Waiting for the first game to be created..."
until [ "$(game_count || echo 0)" -gt 0 ]; do
  sleep 5
done

current_idx="${START_INDEX:-0}"
existing="$(game_count || true)"
if [ -n "${existing}" ] && [ "${existing}" -gt 0 ] && [ -z "${START_INDEX:-}" ]; then
  current_idx=$((existing - 1))
fi

echo "Starting challenger from index ${current_idx} using factory ${GAME_FACTORY_ADDRESS}"

while true; do
  total_games="$(game_count || echo 0)"
  if [ "${current_idx}" -ge "${total_games}" ]; then
    sleep "${RESOLVE_INTERVAL}"
    continue
  fi

  address="$(game_address "${current_idx}")" || {
    sleep "${RESOLVE_INTERVAL}"
    continue
  }
  echo "Processing game ${current_idx} at ${address}"

  attempt=0
  claim_ok=0
  until [ "${attempt}" -ge "${CLAIM_RETRIES}" ]; do
    if op-challenger resolve-claim \
      --l1-eth-rpc "${L1_RPC_URL}" \
      --game-address "${address}" \
      --claim 0 \
      --private-key "${ADMIN_PRIVATE_KEY}"; then
      echo "Resolved claim 0 for ${address}"
      claim_ok=1
      break
    fi

    attempt=$((attempt + 1))
    echo "Game not ready yet, retrying (${attempt}/${CLAIM_RETRIES})..."
    sleep "${CLAIM_RETRY_DELAY}"
  done

  if [ "${claim_ok}" -ne 1 ]; then
    echo "Could not resolve claim for ${address} after retries, will retry later"
    sleep "${CLAIM_RETRY_DELAY}"
    continue
  fi

  if ! op-challenger resolve \
    --l1-eth-rpc "${L1_RPC_URL}" \
    --game-address "${address}" \
    --private-key "${ADMIN_PRIVATE_KEY}"; then
    echo "Resolve failed for ${address}, will retry later"
    sleep "${CLAIM_RETRY_DELAY}"
    continue
  fi

  current_idx=$((current_idx + 1))
  sleep "${RESOLVE_INTERVAL}"
done
