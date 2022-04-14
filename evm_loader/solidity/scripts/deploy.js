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

async function deployERC20(token_list_file) {
  fs.readFile(token_list_file, 'utf8' , async (err, data) => {
    if (err) {
      console.error(err)
      return
    }

    const chainId = hre.network.config.chainId;
    let token_list = JSON.parse(data);
    const NeonERC20Wrapper = await hre.ethers.getContractFactory("NeonERC20Wrapper");

    for (let spl_token of token_list.tokens) {
      if (chainId != spl_token.chainId) {
        continue;
      }

      if (!createSplToken(spl_token)) {
        continue;
      }

      console.log(`Deploying wrapper for SPL token ${spl_token.name} (${spl_token.symbol})`);
      const new_wrapper = await NeonERC20Wrapper.deploy(
          spl_token.name,
          spl_token.symbol,
          base58_to_binary(spl_token.address_spl));

      await new_wrapper.deployed();
      console.log(`   Wrapper deployed: ${new_wrapper.address}`);
      spl_token.address = new_wrapper.address;
    }

    fs.writeFile(
        token_list_file,
        JSON.stringify(token_list, null, ' '),
        function(err) {
          if (err) {
            console.log(err);
          }
        });
  });
}

async function main() {
  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with the account:", deployer.address);

  await deployNeon();
  await deployQueryAccount();
  await deployERC20('./tokenlist.json');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

