# Declare variable for Rust toolchain version which should be filled with `rust-toolchain` file contents
ARG RUST_TOOLCHAIN="1.89.0"

# Start from a container that has workspace rust toolchain
FROM rust:${RUST_TOOLCHAIN}-alpine3.21 AS build

# Set working directory
WORKDIR /volume

RUN \
  # Install build dependencies
  apk add --no-cache clang clang-dev lld curl build-base linux-headers git openssl-dev \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy sources over to the image
COPY . .

# Build release binary
RUN --mount=type=cache,target=/usr/local/cargo/git \
  --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/volume/target \
  # RocksDB breaks the build with libclang.so not found https://github.com/apache/skywalking/issues/10439
  # Aptos-core breaks the build with `disable_lifo_slot` not found https://github.com/aptos-labs/aptos-core/issues/5655 \
  RUSTFLAGS="-C target-feature=-crt-static --cfg tokio_unstable" \
  cargo build --bin op-move --release --features storage,op-upgrade \
  # Copy binary out of target directory, else it's removed by the cache mount
  && mv target/release/op-move /volume/op-move

# Switch to clean image
FROM alpine:3.21.2 AS op-move

# Set working directory
WORKDIR /volume

RUN \
  # Install run dependencies
  apk add --no-cache bash clang lld curl build-base linux-headers git openssl-dev \
  # Clean up
  && rm -rf /tmp/* /var/cache/apk/*

# Copy Move framework snapshot files into expected paths
COPY genesis/aptos.mrb genesis/sui.mrb genesis/

# Copy L2 genesis config into expected path
COPY execution/src/tests/res/l2_genesis_tests.json server/src/tests/optimism/packages/contracts-bedrock/deployments/genesis.json
COPY execution/src/tests/res/bridged_tokens_test.json execution/src/tests/res/bridged_tokens_test.json

# Copy script that waits for other services availability
COPY --chmod=0777 docker/shared/wait-for-it.sh /usr/local/bin/wait-for-it

# Copy entrypoint with run privileges
COPY --chmod=0777 docker/op-move/entrypoint.sh ./

# Copy built binary
COPY --from=build /volume/op-move ./

ENTRYPOINT [ "/volume/entrypoint.sh" ]
