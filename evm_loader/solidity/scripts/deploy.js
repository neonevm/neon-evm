const hre = require("hardhat");
const fs = require("fs");
const { base58_to_binary } = require('base58-js')

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

      console.log(`Deploying wrapper for SPL token ${spl_token.address_spl}: ${spl_token.name} (${spl_token.symbol})`);
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
  await deployNeon();
  await deployQueryAccount();
  await deployERC20('./tokenlist.json');
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

