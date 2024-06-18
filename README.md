# op-move

A Move VM execution layer for OP Stack.

# Testing

Make sure you have the following dependencies installed on your system.

| Dependency | Version | Version Check Command |
|------------|---------|--------------|
| [go](https://go.dev/) | `^1.21` | `go version` |
| [docker](https://docs.docker.com/engine/install/) | `^26`  | `docker -v` |
| [foundry](https://github.com/foundry-rs/foundry#installation) | `^0.2.0` | `forge --version` |

## Optimism

If Optimism deployment fails with import issues, try bringing in the Optimism submodule manually with `git submodule update --init --recursive`
The submodule is used to compile and deploy Optimism contracts.

Make sure the Optimism binaries are built and are in the PATH.

```bash
cd src/tests/optimism
make op-node op-batcher op-proposer
mv op-node/bin/op-node ~/go/bin/
mv op-batcher/bin/op-batcher ~/go/bin/
mv op-proposer/bin/op-proposer ~/go/bin/
```

Similarly, clone the [`op-geth` project](https://github.com/ethereum-optimism/op-geth) and install the `geth` binary, but save it as `op-geth`.
```bash
cd op-geth
make geth
mv build/bin/geth ~/go/bin/op-geth # make sure it's saved as op-geth instead of geth
```

Build and install the Ethereum L1 runner from [`geth` project](https://github.com/ethereum/go-ethereum).
```bash
git clone https://github.com/ethereum/go-ethereum.git
git checkout tags/v1.14.5 # or higher
cd go-ethereum
make geth
mv build/bin/geth ~/go/bin/geth
```

If you notice an error about Optimism fault proof, run the following command in the `optimism` root folder.
```bash
make cannon-prestate
```

To bridge over ETH from L1 to L2, find the `L1StandardBridgeProxy` address in the list of deployed contract addresses file:
`src/tests/optimism/packages/contracts-bedrock/deployments/1337-deploy.json`
