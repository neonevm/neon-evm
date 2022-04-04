require("@nomiclabs/hardhat-waffle");

const neon_token_deployer = '0x4deacb079b4714c38f39508aa8900039f2721ed8686835d43347ba9267da767b';

module.exports = {
  solidity: "0.8.4",
  networks: {
    ci: {
      url: 'http://proxy:9090/solana',
      accounts: [neon_token_deployer],
      network_id: 111,
      chainId: 111,
      gas: 3000000,
      gasPrice: 1000000000000,
      blockGasLimit: 10000000,
      allowUnlimitedContractSize: false,
      timeout: 1000000,
      isFork: true
    },
    devnet: {
      url: 'https://proxy.devnet.neonlabs.org/solana',
      accounts: [neon_token_deployer],
      network_id: 245022926,
      chainId: 245022926,
      gas: 3000000,
      gasPrice: 1000000000000,
      blockGasLimit: 10000000,
      allowUnlimitedContractSize: false,
      timeout: 1000000,
      isFork: true
    },
    testnet: {
      url: 'https://proxy.testnet.neonlabs.org/solana',
      accounts: [neon_token_deployer],
      network_id: 245022940,
      chainId: 245022940,
      gas: 3000000,
      gasPrice: 1000000000000,
      blockGasLimit: 10000000,
      allowUnlimitedContractSize: false,
      timeout: 1000000,
      isFork: true
    }
  }
};
