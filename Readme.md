# op-move

A Move VM execution layer for OP Stack.

# Integration testing

Make sure you have `go` installed on your system. Due to the pinned versions being based around August 2024, a version
not older than 1.22 is required. Other dependencies include [foundry](http://getfoundry.sh/) for smart contract
interaction and [jq](https://jqlang.github.io/jq/) being called indirectly by Optimism itself.

While inside the `op-move` folder, clone the Optimism monorepo. The repo is used to compile and deploy Optimism
contracts.

```bash
git clone https://github.com/ethereum-optimism/optimism server/src/tests/optimism
```

To pull in the libraries required by Optimism, run the following inside that repo:

```bash
cd server/src/tests/optimism
make submodules
```

Make sure the Optimism binaries are built and are in the PATH, i.e. under the `go` path.

```bash
cd server/src/tests/optimism
git checkout v1.7.6
make op-node op-batcher op-proposer
mv op-node/bin/op-node ~/go/bin/
mv op-batcher/bin/op-batcher ~/go/bin/
mv op-proposer/bin/op-proposer ~/go/bin/
```

Build and install the Ethereum L1 runner from the [`geth` project](https://github.com/ethereum/go-ethereum).

```bash
git clone https://github.com/ethereum/go-ethereum.git
cd go-ethereum
git checkout tags/v1.14.6 # or higher
make geth
mv build/bin/geth ~/go/bin/geth
```

# Issues

### Go-Ethereum version

Make sure the `geth` version is compatible. Otherwise, the API communication could fail. The best way to match the
versions is to check out a `go-ethereum` `tag` around the day of the `optimism` commit in submodule.
For instance, a compatible `geth` tag is `tags/v1.14.6` for the current `optimism` version.
To check which commit we use for Optimism:

```bash
cd server/src/tests/optimism
git branch
```

This shows the `(HEAD detached at <commit>)` and find the day the `<commit>` was pushed.

### Fault proof setup

When you run the integration test, if you notice an error about Optimism fault proof, run the following command inside
the `optimism` root folder.

```bash
make cannon-prestate
```

### Stalled process

When you see a message with the address already being used, it means `geth` isn't shutdown correctly from a previous
test run and most likely `geth` is still running in the background.
The integration test cannot shut this down automatically when it starts, so open up Activity Monitor or Task Manager to
force any process with names `geth` or `op-*` to shut down.

### Optimism repo location

Make sure the `optimism` folder is inside the `op-move` project, at `op-move/server/src/tests/optimism`.

# OP upgrade walkthrough

The below steps were tested with following versions: `op-deployer` at 0.4.5, `geth` at 1.16.7, and all the OP stack checked out and built at 1.14.1.

The deployer can have a `--workdir` passed to it as an argument, otherwise it spawns all files in the current directory, the main one being
`state.json`. If we're doing a rerun, it would be useful to cleanup:

```bash
rm *.json
```

Getting rid of `l1_datadir` and `op-move` db files is a given too.

The first deployment step is generating an intent file:

```bash
op-deployer init --l2-chain-ids=42069 --l1-chain-id=1337 --intent-type custom
```

This will create an `intent.toml` that needs to be filled. This is a filled version
with all the owner accounts set to our admin:

```toml
configType = "custom"
l1ChainID = 1337
fundDevAccounts = false
l1ContractsLocator = "embedded"
l2ContractsLocator = "embedded"

[superchainRoles]
SuperchainProxyAdminOwner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
SuperchainGuardian = "0x2611596AA00F8438d75f1daF893CF264366fc668"
ProtocolVersionsOwner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
Challenger = "0x2611596AA00F8438d75f1daF893CF264366fc668"

[[chains]]
id = "0x000000000000000000000000000000000000000000000000000000000000a455"
baseFeeVaultRecipient = "0x2611596AA00F8438d75f1daF893CF264366fc668"
l1FeeVaultRecipient = "0x2611596AA00F8438d75f1daF893CF264366fc668"
sequencerFeeVaultRecipient = "0x2611596AA00F8438d75f1daF893CF264366fc668"
eip1559DenominatorCanyon = 50
eip1559Denominator = 50
eip1559Elasticity = 6
gasLimit = 60000000
operatorFeeScalar = 0
operatorFeeConstant = 0
[chains.roles]
l1ProxyAdminOwner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
l2ProxyAdminOwner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
systemConfigOwner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
unsafeBlockSigner = "0x2611596AA00F8438d75f1daF893CF264366fc668"
batcher = "0x8C67a7B8624044F8F672E9EC374dFa596f01aFB9"
proposer = "0xb846C69FA1f6D2DC86Ee44553f67Bbb86e007d08"
# should it be a separate address?
challenger = "0x2611596AA00F8438d75f1daF893CF264366fc668"

[globalDeployOverrides]
faultGameWithdrawalDelay = 30
preimageOracleChallengePeriod = 15
proofMaturityDelaySeconds = 30
disputeGameFinalityDelaySeconds = 30
faultGameMaxDepth = 44
faultGameSplitDepth = 14
faultGameClockExtension = 7
faultGameMaxClockDuration = 30
```

The final section above makes sure that fault game parameters are not default, and thus
withdrawals can proceed within a reasonable amount of time for the integration test to
pass instead of 3.5 days.

These can be adjusted relatively freely as long as
`max(preimageOracleChallengePeriod + faultGameClockExtension, faultGameClockExtension * 2) <= faultGameMaxClockDuration`
which is enforced by OPCM during contract init, otherwise it reverts.

After this, all the steps to deploy contracts on the L1 follow:

```bash
op-deployer bootstrap proxy --l1-rpc-url=http://127.0.0.1:58138 --private-key=0x3ae90739336cd848513adb4a5d6cae372b64135fb4d214aa1a25948a21c7b7fd  --proxy-owner 0x2611596AA00F8438d75f1daF893CF264366fc668 --outfile proxy-output.json

op-deployer bootstrap superchain --l1-rpc-url=http://127.0.0.1:58138 --private-key=0x3ae90739336cd848513adb4a5d6cae372b64135fb4d214aa1a25948a21c7b7fd  --superchain-proxy-admin-owner 0x2611596AA00F8438d75f1daF893CF264366fc668 --protocol-versions-owner 0x2611596AA00F8438d75f1daF893CF264366fc668 --guardian 0x2611596AA00F8438d75f1daF893CF264366fc668  --outfile superchain-output.json

op-deployer bootstrap implementations --l1-rpc-url=http://127.0.0.1:58138 --private-key=0x3ae90739336cd848513adb4a5d6cae372b64135fb4d214aa1a25948a21c7b7fd    --outfile bootstrap_implementations.json --superchain-config-proxy 0x3000f5214879c31a203d5df693f7e7c2850d52f5   --protocol-versions-proxy  0x45af18faf3ec5fa0401cded0e59c21377adc689e     --upgrade-controller 0x2611596AA00F8438d75f1daF893CF264366fc668  --challenger 0x2611596AA00F8438d75f1daF893CF264366fc668 --superchain-proxy-admin 0x481733903ad24403dd6dabc00ce4db6789a0c41f
```

Most of the addresses/keys are the usual admin ones all over. Of note here is the final step only, as it needs other addresses as well.
Protocol versions, superchain config proxy and superchain proxy admin come from the superchain bootstrapping file, i.e. `superchain-output.json`:

```json
 {
   "protocolVersionsImplAddress": "0x37e15e4d6dffa9e5e320ee1ec036922e563cb76c",
   "protocolVersionsProxyAddress": "0x45af18faf3ec5fa0401cded0e59c21377adc689e",
   "superchainConfigImplAddress": "0xce28685eb204186b557133766eca00334eb441e4",
   "superchainConfigProxyAddress": "0x3000f5214879c31a203d5df693f7e7c2850d52f5",
   "proxyAdminAddress": "0x481733903ad24403dd6dabc00ce4db6789a0c41f"
 }
 
```

Next, the deployer needs to apply the actual intent:

```bash
op-deployer apply --l1-rpc-url=http://127.0.0.1:58138 --private-key=0x3ae90739336cd848513adb4a5d6cae372b64135fb4d214aa1a25948a21c7b7fd
```

As noted before, this only seems to work after the L1 safe head has advanced quite a bit, i.e. when geth has produced about 150 blocks.

Now we can generate the genesis and rollup files:

```bash
op-deployer inspect rollup 42069 > rollup.json
op-deployer inspect genesis 42069 > genesis.json
```

These are need to run `op-node` and `op-move` respectively.

```bash
op-deployer inspect l1 42069 > l1.json
cp l1.json ../../../res/
```

As the new addresses are changing with each deployment, the revamped integration test
expects a fresh version of the L1 addresses file in its `res/` subdir.

Some other useful addresses (e.g. deployment config) can be generated with other `op-deployer inspect ...` subcommands.

The updated commands to run the components are as follows.

The JWT secret is assumed to be stored in a file. It can be generated with a following command:

```bash
openssl rand -hex 32 > jwt.txt
```

So that it outputs a hex string like `cc7b33beae0918ac5e963db79edd09265a7792c2ebfa8221d9bc3ec214b8e27b`.

Any old state before running the binary should be deleted, i.e. `rm -rf db` needs to be run in the root dir or
the binary run with `OP_MOVE_DB_PURGE=1` env var.

```bash
cargo r --bin op-move --features storage,op-upgrade --release -- --genesis.l2-contract-genesis genesis.json --auth.jwt-secret $(cat jwt.txt)
```

As the state root of the genesis generated by `op-deployer` will always be different, the first run of `op-move` will fail with a mismatch.
The value on the left is the new one that should be inserted into `genesis/src/config.rs` or it can be obtained from `genesis.json` preemptively.
The db needs to be wiped again before the second run if it happens.

Newer versions of `op-node` also need the L1 genesis to run. If `geth` node is already running, it's useful to copy the datadir
first as `geth` doesn't permit to generate genesis from a dir that is being currently used:

```bash
cp -r l1_datadir/ /tmp/geth-copy
geth dumpgenesis --datadir /tmp/geth-copy > l1_genesis.json
```

The command for `geth` is exactly same as before.

```bash
geth --dev --dev.period 3 --datadir ./l1_datadir --rpc.allow-unprotected-txs --http --http.addr 0.0.0.0 --http.port 58138 --http.corsdomain "*" --http.api web3,debug,eth,txpool,net,engine
```

We also need to prefund the admin/batcher etc. accounts and deploy the optimism factory contract.
The actual prefunded geth dev address may be different. Alternatively it can be done just like in docker deployment by using the keystore in L1 datadir.
Another point is that `op-deployer apply` only works after L1 safe head has sufficiently progressed, so we can repeat funding txs in a loop to reach
~150 blocks:

```bash

for i in {1..30}; do
  cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 10ether 0x2611596AA00F8438d75f1daF893CF264366fc668
  cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 10ether  0x7111d029cDD94Eaed215439F9269564Cf9dCE403
  cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 10ether 0x89D740330E773E42edF98Bba1D8D1D6C545D78a6
  cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 10ether   0x8C67a7B8624044F8F672E9EC374dFa596f01aFB9
  cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 10ether 0xb846C69FA1f6D2DC86Ee44553f67Bbb86e007d08
done

cast send --rpc-url http://localhost:58138 --from 0x71562b71999873db5b286df957af199ec94617f7 --unlocked --value 1ether 0x3fAB184622Dc19b6109349B94811493BF2a45362

cast publish --rpc-url localhost:58138 0xf8a58085174876e800830186a08080b853604580600e600039806000f350fe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe03601600081602082378035828234f58015156039578182fd5b8082525050506014600cf31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222

```

For node only last flag is new:

```bash
op-node \
  --l1 http://localhost:58138 \
  --l1.beacon.ignore \
  --l1.rpckind basic \
  --l2 http://localhost:8551 \
  --l2.jwt-secret jwt.txt \
  --sequencer.enabled \
  --sequencer.l1-confs 5 \
  --verifier.l1-confs 4 \
  --rollup.config rollup.json \
  --rpc.addr 0.0.0.0 \
  --rpc.port 8547 \
  --p2p.disable \
  --rpc.enable-admin \
  --p2p.sequencer.key 0x66fe1b9d3babc69ad68fad96ac55b08f8d0ab3cb404c78538c0a9616ad9b1d3e \
  --rollup.l1-chain-config l1_genesis.json
```

For batcher only last flag is new too:

```bash
op-batcher \
  --l2-eth-rpc http://localhost:8545 \
  --rollup-rpc  http://localhost:8547 \
  --poll-interval 1s \
  --sub-safety-margin 6 \
  --num-confirmations 1 \
  --safe-abort-nonce-too-low-count 3 \
  --resubmission-timeout 30s \
  --rpc.addr 0.0.0.0 \
  --rpc.port 8548 \
  --rpc.enable-admin \
  --max-channel-duration 1 \
  --private-key  0x20635cd468a1c32892bbde520be598519ae5141992b6d4fb7d7dd54126ff0511 \
  --l1-eth-rpc http://localhost:58138 \
  --throttle.unsafe-da-bytes-lower-threshold 0
```

The new proposer needs game factory instead of `L2OutputOracleProxy`. It can be found as such in the folder where `op-deployer`
was run:

```bash
cat state.json | jq -r '.opChainDeployments[0].DisputeGameFactoryProxy'
```

With proposer, the game factory contract now replaces the functions of L2OutputOracle. Game type
and proposal interval are needed because of that new flag as well. The first is set to permissioned
games which are deployed by default, and proposal interval signifies how often the proposer tries to submit
new output roots to L1. Sometimes it happens to try submitting the same output roots in a row many times
despite the game already being created, so to prevent that having a slightly higher interval is ideal.

```bash
op-proposer \
  --poll-interval 12s \
  --rpc.port 8560 \
  --rpc.enable-admin \
  --rollup-rpc http://localhost:8547 \
  --private-key 0x589a8c172e0a5bd75a2d55e4ef71470bbf06776e7ab17124fb94cb92163bb05d \
  --game-type 1 \
  --game-factory-address $GAME_FACTORY_ADDRESS \
  --l1-eth-rpc http://localhost:58138 \
  --num-confirmations 1 \
  --allow-non-finalized true \
  --proposal-interval 1m
```

This might be superfluous, but by default permissioned games have their
[bond](https://docs.optimism.io/op-stack/fault-proofs/challenger#what-is-the-bond-what-is-the-bond%E2%80%99s-role-in-the-fp-system)
at 0 so it might be a good practice to set it to a reasonable amount, though
proposer seems to work fine without it:

```bash
cast send $GAME_FACTORY_ADDRESS 'setInitBond(uint32,uint256)' 1 8000000000000000 --rpc-url localhost:58138 --private-key 0x3ae90739336cd848513adb4a5d6cae372b64135fb4d214aa1a25948a21c7b7fd
```

As a final step, though we can't directly run `op-challenger` as a daemon (due to it requiring a beacon node setup), we can still
use it for open games resolution with consequent bond redistribution etc. So it is expected to be built and available in `$PATH`, i.e.

```bash
cd server/src/tests/optimism
make op-challenger
mv op-challenger/bin/op-challenger ~/go/bin/
```
