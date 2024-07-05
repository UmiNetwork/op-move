# op-move

A Move VM execution layer for OP Stack.

# Integration testing

Make sure you have `go` installed on your system.

Optimism monorepo is pulled in as a submodule of this repo. The submodule is used to compile and deploy Optimism contracts.
If you don't see it inside the test folder, try bringing in the Optimism submodule manually
```bash
git submodule update --init --recursive
```

Make sure the Optimism binaries are built and are in the PATH, ie under the `go` path.
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

Build and install the Ethereum L1 runner from the [`geth` project](https://github.com/ethereum/go-ethereum).
```bash
git clone https://github.com/ethereum/go-ethereum.git
git checkout tags/v1.14.5 # or higher
cd go-ethereum
make geth
mv build/bin/geth ~/go/bin/geth
```

When you run the integration test, if you notice an error about Optimism fault proof, run the following command inside the `optimism` root folder.
```bash
make cannon-prestate
```
