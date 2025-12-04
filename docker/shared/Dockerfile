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
FROM golang:1.24-alpine AS optimism

# Copy forge from foundry to install dependencies for contracts
COPY --from=foundry /forge /usr/local/bin/

# Copy only the specific Solidity compiler versions needed
COPY --from=ethereum/solc:0.8.15 /usr/bin/solc /root/.svm/0.8.15/solc-0.8.15
COPY --from=ethereum/solc:0.8.19 /usr/bin/solc /root/.svm/0.8.19/solc-0.8.19
COPY --from=ethereum/solc:0.8.25 /usr/bin/solc /root/.svm/0.8.25/solc-0.8.25
COPY --from=ethereum/solc:0.8.30 /usr/bin/solc /root/.svm/0.8.30/solc-0.8.30

RUN \
  # Install build dependencies
  apk add --no-cache git make bash just \
  # Clone optimism at specific version (options must precede repo/target path)
  && git clone --branch v1.16.2 --depth 1 https://github.com/ethereum-optimism/optimism /volume \
  # Build optimism binaries and files required for op-node startup
  && make --directory /volume op-node op-batcher op-proposer op-challenger \
  # Build op-deployer for L1 deployment separately as it's not part of the main Makefile
  && cd /volume/op-deployer && just build \
  && cd / \
  # Move build artifacts to the root directory
  && mv \
  # Only the built binaries we need
  /volume/op-node/bin/op-node \
  /volume/op-batcher/bin/op-batcher \
  /volume/op-proposer/bin/op-proposer \
  /volume/op-challenger/bin/op-challenger \
  /volume/op-deployer/bin/op-deployer \
  / \
  # Clean up
  && rm -rf $(ls -d /* /etc/* | grep -v \
  -e ^/op-node$ -e ^/op-batcher$ -e ^/op-proposer$ -e ^/op-challenger$ \
  -e ^/op-deployer$ \
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
COPY --from=optimism /op-proposer /usr/local/bin/

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-proposer/entrypoint.sh ./
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
COPY --from=optimism /op-batcher /usr/local/bin/

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-batcher/entrypoint.sh ./
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from an image that already includes Go 1.24, which matches the go.mod requirement
FROM golang:1.24-alpine AS go-ethereum

RUN \
  # Install build dependencies
  apk add --no-cache git make linux-headers \
  # Clone geth sources
  && git clone https://github.com/ethereum/go-ethereum /volume --branch v1.16.7 --depth 1 \
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
ENV L1_DATADIR="/volume/l1_datadir"
ENV SIGNED_L1_CONTRACT_TX="0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222"
ENV FACTORY_DEPLOYER_ADDRESS="0x3fAB184622Dc19b6109349B94811493BF2a45362"

RUN \
  # Install build dependencies
  apk upgrade --no-cache \
  && apk add --no-cache bash curl jq git \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy only the specific Solidity compiler versions needed
COPY --from=ethereum/solc:0.8.15 /usr/bin/solc /root/.svm/0.8.15/solc-0.8.15
COPY --from=ethereum/solc:0.8.19 /usr/bin/solc /root/.svm/0.8.19/solc-0.8.19
COPY --from=ethereum/solc:0.8.25 /usr/bin/solc /root/.svm/0.8.25/solc-0.8.25
COPY --from=ethereum/solc:0.8.30 /usr/bin/solc /root/.svm/0.8.30/solc-0.8.30

# Copy files for Optimism L1 deployment and L2 genesis initialization
COPY --from=optimism /op-deployer /usr/local/bin/

# Copy built binaries of foundry tools
COPY --from=foundry /cast /forge /usr/local/bin/

# Copy built binary from build image
COPY --from=go-ethereum /geth /usr/local/bin/

# Copy scripts and genesis config generator with run privileges
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it
COPY --chmod=0777 docker/geth/*.sh server/src/tests/res/filled_intent.toml ./

# Set entrypoint to a script that launches geth
ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from a clean image
FROM alpine:3.21.2 AS op-node
WORKDIR /volume

# Copy cast from foundry to wait for op-move
COPY --from=foundry /cast /usr/local/bin/

# Copy built binary from build image
COPY --from=optimism /op-node /usr/local/bin/

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-node/entrypoint.sh ./

ENTRYPOINT [ "/volume/entrypoint.sh" ]

# Start from a clean image
FROM alpine:3.21.2 AS op-node-rep
WORKDIR /volume

RUN \
  # Install build dependencies
  # bash required by wait-for-it
  # curl required for calling p2p RPC
  apk upgrade --no-cache \
  && apk add --no-cache bash curl \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy cast from foundry to wait for op-move
COPY --from=foundry /cast /usr/local/bin/

# Copy built binary from build image
COPY --from=optimism /op-node /usr/local/bin/

# Copy script that waits for other services availability
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-node-rep/entrypoint.sh ./

ENTRYPOINT [ "/volume/entrypoint.sh" ]
