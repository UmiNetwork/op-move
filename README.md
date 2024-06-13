# op-move

A Move VM execution layer for OP Stack.

# Testing

Make sure you have the following dependencies installed on your system.

| Dependency | Version | Version Check Command |
|------------|---------|--------------|
| [go](https://go.dev/) | `^1.21` | `go version` |
| [docker](https://docs.docker.com/engine/install/) | `^26`  | `docker -v` |
| [foundry](https://github.com/foundry-rs/foundry#installation) | `^0.2.0` | `forge --version` |
| [kurtosis](https://docs.kurtosis.com/install#ii-install-the-cli) | `^0.89` | `kurtosis version` |

## Kurtosis

Kurtosis will start an Ethereum local testnet in a Docker container.
The node configuration is in `./src/tests/kurtosis.yaml` and can be extended.
For instance, currently the block time is set to 2 seconds and if you want to change the block time, you should update both `kurtosis.yaml` and also `config.sh`.

```bash
kurtosis clean -a && kurtosis run --enclave local-testnet github.com/kurtosis-tech/ethereum-package --args-file ./src/tests/kurtosis.yaml
```

After running the Ethereum node, make sure to update `L1_RPC_URL` in the `.env` file with the corresponding exposed RPC URL.
For instance, the Kurtosis output will display the exposed User Services and the exposed RPC port for `8545`.
`8545` port is the internal Docker port, so we want the right side of the arrow -> `rpc: 8545/tcp -> http://127.0.0.1:<port>` and copy the `<port>` value into the `.env` file.

Add the Kurtosis local Ethereum testnet as a network on your MetaMask wallet using the following settings:

| Add Network     | Values                  |
|-----------------|-------------------------|
| Network name    | Ethereum Devnet         |
| New RPC URL     | http://127.0.0.1:<port> |
| Chain ID        | 3151908                 |
| Currency symbol | ETH                     |

Add one of the Kurtosis test accounts with pre-funded ETH, so you can send ETH to other accounts.
Here's the list of [pre-funded accounts](https://github.com/kurtosis-tech/ethereum-package/blob/main/src/prelaunch_data_generator/genesis_constants/genesis_constants.star) 
and you can use the first private key: `bcdf20249abf0ed6d944c0288fad489e33f66b3960d9e6229c1cd214ed3bbe31`.

## Optimism

If Optimism deployment fails with import issues, try bringing in the Optimism submodule with `git submodule update --init --recursive`
The submodule is used to compile and deploy Optimism contracts.

Make sure the Optimism binaries are built and are in the PATH.

```bash
cd src/tests/optimism
make op-node op-batcher op-proposer
mv op-node/bin/op-node ~/go/bin/ # repeat for op-batcher and op-proposer
```

Similarly, clone the [`op-geth` project](https://github.com/ethereum-optimism/op-geth) and install the `geth` binary.

To bridge over ETH from L1 to L2, find the `L1StandardBridgeProxy` address in the list of deployed contract addresses file:
`src/tests/optimism/packages/contracts-bedrock/deployments/3151908-deploy.json`
