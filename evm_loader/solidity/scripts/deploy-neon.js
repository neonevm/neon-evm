const hre = require("hardhat");

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

async function deployERC20(spl_tokens) {
  const ERC20Wrapper = await hre.ethers.getContractFactory("NeonERC20Wrapper");

  for (const spl_token of spl_tokens) {
    const new_wrapper = await ERC20Wrapper.deploy(
        spl_token.token_name,
        spl_token.token_symbol,
        spl_token.token_mint);

    await new_wrapper.deployed();
    console.log(`SPL Token ${spl_token.token_mint}: ${spl_token.token_name} (${spl_token.token_symbol}) wrapper deployed at ${new_wrapper.address}`);
  }
}

async function main() {
  const Neon = await hre.ethers.getContractFactory("NeonToken");
  const neon = await Neon.deploy();

  await neon.deployed();
  console.log("Neon contract address is: ", neon.address);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
