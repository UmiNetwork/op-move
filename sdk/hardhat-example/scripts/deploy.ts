import {AccountAddress, EntryFunction, TransactionPayloadEntryFunction} from "@aptos-labs/ts-sdk";
import { ethers } from 'hardhat';
import { TransactionRequest } from "ethers";

async function main() {
    const [deployer] = await ethers.getSigners();
    console.log('Deploying contracts for the account:', deployer.address);

    const Counter = await ethers.getContractFactory('counter');
    const counter = await Counter.deploy();
    await counter.waitForDeployment();
    const moduleAddress = await counter.getAddress();
    console.log('Counter address:', moduleAddress);

    // Use the deployed module
    const bobAddress = '0xb0b';
    const entryFunction = EntryFunction.build(
      `${moduleAddress}::counter`,
      'get_count',
      [], // Use `parseTypeTag(..)` to get type arg from string
      [AccountAddress.fromString(bobAddress)],
    );
    const transactionPayload = new TransactionPayloadEntryFunction(entryFunction);
    const payload = transactionPayload.bcsToHex();
    const request: TransactionRequest = {
        to: moduleAddress,
        data: payload.toString(),
    };
    await deployer.sendTransaction(request);
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });
