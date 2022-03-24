const hre = require("hardhat");

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
