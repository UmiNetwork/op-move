#!/bin/bash
# Entrypoint of the op-batcher docker container

# -e Exit if a command fails
# -u Treat unset or undefined variables as errors
# -x Print out command arguments during execution
set -eux
L1_DATADIR="./l1_datadir"
KEYFILE="89d740330e773e42edf98bba1d8d1d6c545d78a6"

# In case of restart, data dir will not be empty
# Wipe it to avoid an attempt to restore the state which is not persisted in `--dev` mode
rm -rf "${L1_DATADIR}" && mkdir -p "${L1_DATADIR}" && mkdir -p "${L1_DATADIR}/keystore"
echo '{"address":"89d740330e773e42edf98bba1d8d1d6c545d78a6","crypto":{"cipher":"aes-128-ctr","ciphertext":"755bbdc3f7bed78a5434fcce4dc118795dd049b0538084c3ce12a91035b32c60","cipherparams":{"iv":"22e00eec04c8d117ad12c0b2924f6a6b"},"kdf":"scrypt","kdfparams":{"dklen":32,"n":4096,"p":6,"r":8,"salt":"cab7899fd54ef0164f572aa41b3941d366ccd38ec624d06ee4e8ee088ef4da3d"},"mac":"0b0a2137144d1efdea0083a22e265300a11bf0d3e5b55285f4ce366a8eb1f8bf"},"id":"52f30552-f758-4cd0-97d4-7f957b51ef30","version":3}' > "${L1_DATADIR}/keystore/${KEYFILE}"

./geth-init.sh &

# Ephemeral proof-of-authority network with a pre-funded developer account,
# with automatic mining when there are pending transactions.
geth \
    --dev \
    --dev.period 3 \
    --datadir "${L1_DATADIR}" \
    --rpc.allow-unprotected-txs \
    --http \
    --http.addr 0.0.0.0 \
    --http.port 58138 \
    --http.corsdomain '*' \
    --http.api 'web3,debug,eth,txpool,net,engine' \
    --http.vhosts '*'
