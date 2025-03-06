#!/bin/sh
# Serve with the same keystore file so the faucet address doesn't change

KEYSTORE_DIR="./l1_datadir/keystore"
KEYFILE="UTC--2025-01-01T00-00-00.000000000Z--89d740330e773e42edf98bba1d8d1d6c545d78a6"

mkdir -p "${KEYSTORE_DIR}"

cat > "${KEYSTORE_DIR}/${KEYFILE}" <<EOL
{
  "address": "89d740330e773e42edf98bba1d8d1d6c545d78a6",
  "crypto": {
    "cipher": "aes-128-ctr",
    "ciphertext": "755bbdc3f7bed78a5434fcce4dc118795dd049b0538084c3ce12a91035b32c60",
    "cipherparams": { "iv": "22e00eec04c8d117ad12c0b2924f6a6b" },
    "kdf": "scrypt",
    "kdfparams": {
      "dklen": 32,
      "n": 4096,
      "p": 6,
      "r": 8,
      "salt": "cab7899fd54ef0164f572aa41b3941d366ccd38ec624d06ee4e8ee088ef4da3d"
    },
    "mac": "0b0a2137144d1efdea0083a22e265300a11bf0d3e5b55285f4ce366a8eb1f8bf"
  },
  "id": "52f30552-f758-4cd0-97d4-7f957b51ef30",
  "version": 3
}

EOL