const hre = require("hardhat");
const fs = require("fs");
const { base58_to_binary } = require('base58-js')
const { execSync } = require("child_process");

const solana_url = process.env.SOLANA_URL;
const spl_token_authority = process.env.SPL_TOKEN_AUTHORITY;

function createSplToken(spl_token) {
  console.log(`\n\n\nCreating SPL token ${spl_token.symbol}...`);
  let token_keyfile = `./ci-tokens/${spl_token.symbol}.json`;
  if (!fs.existsSync(token_keyfile)) {
    console.log(`Keyfile ${token_keyfile} not found. Will skip token creation.`);
    return true;
  }
  console.log(`Token keyfile is ${token_keyfile}`);

  try {
    spl_token.address_spl = String(execSync(`solana address -k "${token_keyfile}"`)).trim();
    console.log(`SPL token address is ${spl_token.address_spl}`)

    stdout = execSync(`spl-token --url ${solana_url} create-token --owner ${spl_token_authority} -- "${token_keyfile}"`);
    console.log(`SPL token ${spl_token.symbol} created: ${spl_token.address_spl}`);
    return true;
  } catch (e) {
    console.log(`Failed to create SPL token ${spl_token.symbol}: ${e}`);
    return false;
  }
}

async function deployNeon() {
  const Neon = await hre.ethers.getContractFactory("NeonToken");
  const neon = await Neon.deploy();

  await neon.deployed();
  console.log("Neon contract address is: ", neon.address);
}

async function deployQueryAccount() {
  const QueryAccount = await hre.ethers.getContractFactory("QueryAccount");
  const queryAccount = await QueryAccount.deploy();

  await queryAccount.deployed();
  console.log("QueryAccount library address is: ", queryAccount.address);
}


async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with the account:", deployer.address);

  await deployNeon();
  await deployQueryAccount();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

