# Start from a clean image
FROM alpine:3.21.2 AS foundry

# Declare argument for target architecture supplied automatically by the build system
ARG TARGETARCH

RUN \
  # Download and extract foundry binaries
  if [ "$TARGETARCH" = "amd64" ]; then \
    wget https://github.com/foundry-rs/foundry/releases/download/v1.2.3/foundry_v1.2.3_alpine_amd64.tar.gz && \
    tar -xzf foundry_v1.2.3_alpine_amd64.tar.gz; \
  elif [ "$TARGETARCH" = "arm64" ]; then \
    wget https://github.com/foundry-rs/foundry/releases/download/v1.2.3/foundry_v1.2.3_alpine_arm64.tar.gz && \
    tar -xzf foundry_v1.2.3_alpine_arm64.tar.gz; \
  else \
    echo "Unsupported architecture"; exit 1; \
  fi \
  # Clean up
  && rm -rf $(ls -d /* /etc/* | grep -v -e ^/cast$ -e ^/forge$ -e ^/sys$ -e ^/proc$ -e ^/dev -e ^/etc$ -e ^/etc/hosts$ -e ^/etc/resolv.conf$)

# Start from an image with go 1.22, the most recent version that works with this version of Optimism
FROM golang:1.22-alpine AS optimism

# Copy forge from foundry to install dependencies for contracts
COPY --from=foundry /forge /usr/local/bin/

RUN \
  # Install build dependencies
  apk add --no-cache git make bash \
  # Clone optimism at specific version
  && git clone https://github.com/ethereum-optimism/optimism /volume --branch v1.7.6 --depth 1 \
  # Build optimism binaries and files required for op-node startup
  && make --directory /volume cannon-prestate op-node op-batcher op-proposer \
  # Install forge dependencies for contracts
  && forge install --root /volume/packages/contracts-bedrock \
  # Move build artifacts to the root directory
  && mv \
    # Only the built binaries we need
    /volume/op-node/bin/op-node \
    /volume/op-batcher/bin/op-batcher \
    /volume/op-proposer/bin/op-proposer \
    # Files required for op-node startup
    /volume/op-program/bin/prestate.json \
    /volume/op-program/bin/meta.json \
    /volume/op-program/bin/prestate-proof.json \
    # Contracts and dependencies needed for OP Stack deployment
    /volume/packages/contracts-bedrock \
    # Git directory to keep the right commit checked out
    /volume/.git / \
  # Clean up
  && rm -rf $(ls -d /* /etc/* | grep -v \
    -e ^/op-node$ -e ^/op-batcher$ -e ^/op-proposer$ \
    -e ^/prestate.json$ -e ^/meta.json$ -e ^/prestate-proof.json$ \
    -e ^/contracts-bedrock$ -e ^/.git$ \
    -e ^/sys$ -e ^/proc$ -e ^/dev -e ^/etc$ -e ^/etc/hosts$ -e ^/etc/resolv.conf$ \
  )

# Start from a clean image
FROM alpine:3.21.2 AS op-proposer
WORKDIR /volume

RUN \
  # bash required by wait-for-it
  apk add --no-cache bash \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy built binary from build image
COPY --from=optimism /op-proposer /usr/local/bin/op-proposer

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-proposer/entrypoint.sh entrypoint.sh
COPY .env .env
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from a clean image
FROM alpine:3.21.2 AS op-batcher
WORKDIR /volume

RUN \
  # bash required by wait-for-it
  apk add --no-cache bash \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy built binary from build image
COPY --from=optimism /op-batcher /usr/local/bin/op-batcher

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-batcher/entrypoint.sh entrypoint.sh
COPY .env .env
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from a clean image
FROM alpine:3.21.2 AS op-node
WORKDIR /volume

RUN \
  # jq required at least by optimism deploy script
  apk add --no-cache clang lld curl build-base linux-headers git bash jq \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-node/entrypoint.sh entrypoint.sh

# Copy genesis config generator with run privileges
COPY --chmod=0777 server/src/tests/config.sh config.sh

# Copy ENV file that include Optimism wallet addresses
COPY .env .env

# Copy script that waits for other services availability
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

# Copy only the specific Solidity compiler versions needed
COPY --from=ethereum/solc:0.8.15 /usr/bin/solc /root/.svm/0.8.15/solc-0.8.15
COPY --from=ethereum/solc:0.8.19 /usr/bin/solc /root/.svm/0.8.19/solc-0.8.19
COPY --from=ethereum/solc:0.8.25 /usr/bin/solc /root/.svm/0.8.25/solc-0.8.25

# Copy built binary from build image and other dependencies
COPY --from=optimism /op-node /usr/local/bin/op-node
COPY --from=optimism /contracts-bedrock /volume/packages/contracts-bedrock
COPY --from=optimism /.git /volume/.git
COPY --from=optimism /prestate.json /volume/op-program/bin/prestate.json
COPY --from=optimism /meta.json /volume/op-program/bin/meta.json
COPY --from=optimism /prestate-proof.json /volume/op-program/bin/prestate-proof.json
COPY --from=foundry /cast /usr/local/bin/cast
COPY --from=foundry /forge /usr/local/bin/forge

ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from an image that can install go 1.22, the most recent working version for this geth version
FROM alpine:3.20.7 AS go-ethereum

RUN \
  # Install build dependencies
  apk add --no-cache git make go linux-headers \
  # Clone geth sources
  && git clone https://github.com/ethereum/go-ethereum /volume --branch v1.14.6 --depth 1 \
  # Build geth
  && make --directory /volume geth \
  # Move geth binary to root directory
  && mv /volume/build/bin/geth /geth \
  # Clean up
  && rm -rf $(ls -d /* /etc/* | grep -v -e ^/geth$ -e ^/sys$ -e ^/proc$ -e ^/dev$ -e ^/etc$ -e ^/etc/hosts$ -e ^/etc/resolv.conf$)

# Start from a clean image
FROM alpine:3.21.2 AS geth
WORKDIR /volume
ENV L1_RPC_ADDR="127.0.0.1"
ENV L1_RPC_PORT="58138"
ENV L1_RPC_URL="http://${L1_RPC_ADDR}:${L1_RPC_PORT}"
ENV SIGNED_L1_CONTRACT_TX="0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222"
ENV FACTORY_DEPLOYER_ADDRESS="0x3fAB184622Dc19b6109349B94811493BF2a45362"

RUN \
  # Install build dependencies
  apk upgrade --no-cache \
  && apk add --no-cache bash curl jq \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy built binary from build image
COPY --from=go-ethereum /geth /usr/local/bin/
COPY --from=foundry /cast /forge /usr/local/bin/
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it
COPY --chmod=0777 docker/geth/*.sh ./
COPY .env ./

RUN \
  # Create datadir for geth
  mkdir -p ./l1_datadir \
  # Initialize keystore in the datadir
  && ./keystore.sh \
  # Launch geth node to receive transactions
  && geth \
  --dev \
  --dev.period 3 \
  --datadir "./l1_datadir" \
  --rpc.allow-unprotected-txs \
  --http \
  --http.addr 0.0.0.0 \
  --http.port 58138 \
  --http.corsdomain '*' \
  --http.api 'web3,debug,eth,txpool,net,engine' \
  --http.vhosts '*' \
  # Store geth process ID
  & GETH_PID=$! \
  # Send transactions that prefund accounts and deploy Optimism factory deployer
  && ./init.sh \
  # Send interrupt signal to geth to shut it down gracefully preventing datadir corruption
  && kill -SIGINT "${GETH_PID}" \
  # Wait for geth to exit
  && wait "${GETH_PID}"

# Set entrypoint to a script that launches geth
ENTRYPOINT [ "/volume/entrypoint.sh" ]
