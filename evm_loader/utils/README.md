# Permissions management scripts

Scripts **set_single_acct_permission.sh, set_many_accts_permission.sh** are suited to operate with client's and contract's permission token balances - to grant or deprive permissions.

## Requirements

To use this scripts one should have next CLI utilities installed:
    - solana CLI interface
    - neon-cli 
    - spl-token program

Non-zero SOL balance is needed on selected Solana network on default account ~/.config/solana/id.json
Also, user should have correct Mint-authority JSON file

## Permission management for a single client/contract

set_single_acct_permission.sh operates with permission tokens balances of a signle client/contract. Script will estimate current state of client's/contract's permission tokens balances and mint selected tokens if needed. One should set environment variables before running this script:
- SOLANA_URL - URL of Solana RPC endpoing (also, default replacements supported: mainnet-beta, testnet, devnet, localhost)
- EVM_LOADER - address of solana account where EVM is deployed to selected Solana network
- MINT_AUTHORITY_FILE - path to JSON file on the local filesystem where private key to mint-authority stored
- OPERATION (either allow|deny) - keyword determining what action should be performed on client's/contract's permissions
- ACCOUNT_TYPE (either client|contract) - keyword determining what type of account (client/contract) actually passed to script
- NEON_ETH_ADDRESS - NEON account address to change access (Eth-compatible - 0x0f45....)
    
Script running without arguments

## Permission management for several clients/contracts

set_many_accts_permission.sh operates with permission tokens balances of several clients/contracts. One should set environment variables before running this script
- SOLANA_URL - URL of Solana RPC endpoing (also, default replacements supported: mainnet-beta, testnet, devnet, localhost)
- EVM_LOADER - address of solana account where EVM is deployed to selected Solana network
- MINT_AUTHORITY_FILE - path to JSON file on the local filesystem where private key to mint-authority stored
- OPERATION (either allow|deny) - keyword determining what action should be performed on client's/contract's permissions
- ACCOUNT_TYPE (either client|contract) - keyword determining what type of account (client/contract) actually passed to script
- ADDRESS_LIST_FILE - path to the file on a local filesystem containing list of client/contract account addresses written each one on a next line (without any punktuation marks)

Script running without arguments

This script will call set_single_acct_permission.sh for every account from <address_list_file>
Failed account ids will be collected in output file with name <address_list_file>.err this file will have the same format as input file.

## Running with docker-compose

### Requirements
docker, docker-compose installed. It is recommended to use latest versions
NOTE: All operations are performed from evm_loader/utils directory

### Setup
docker-compose.yml file contains two services: set_single_acct_permission, set_many_accts_permission each responsible for running corresponding script described above. One should change environment variables (section 'environment') and pathes to keypair/id-list files (section 'volumes') according to his needs. Some variables are not supposed to be edited it is marked by comment.

### Running commands
Setting up permissions for a single client/contract:
  > docker-compose up set_single_acct_permission
  
Setting up permissions for several clients/contracts:
  > docker-compose up set_many_accts_permission
  

