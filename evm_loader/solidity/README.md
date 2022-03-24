# Solidity contracts


This directory contains solidity contracts implementing API to interact with Neon from Solidity:

- NeonERC20Wrapper - ERC20 interface to SPL tokens
- QueryAccount - Interface to get information from Solana accounts
- NeonToken - interface to interact with Neon token from Solidity contracts

# Compilation

Run from current directory
> npx hardhat compile

# Deployment

## NeonToken

Run command:
> npx hardhat run --network <network_name> scripts/deploy-neon.js

Select network_name from one of the following:
- ci_neon_token
- devnet_neon_token
- testnet_neon_token

depending on where you want to use the contract. The idea is that contract should have the same address on all the networks so that it should be deloyed from the same deployer with the same nonces on all networks 