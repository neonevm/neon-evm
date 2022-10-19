const hre = require("hardhat");
const fs = require("fs");
const { base58_to_binary } = require('base58-js')
const { execSync } = require("child_process");

const solana_url = process.env.SOLANA_URL;
const spl_token_authority = process.env.SPL_TOKEN_AUTHORITY;

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

