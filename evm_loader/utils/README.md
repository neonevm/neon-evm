# Permissions management scripts

Scripts mint_permission_token.sh mint_many.sh are suited to operate with client's and contract's permission token balances - to grant or deprive permissions.

## Requirements

To use this scripts one should have next CLI utilities installed:
    - solana CLI interface
    - neon-cli 
    - spl-token program

Non-zero SOL balance is needed on selected Solana network on default account ~/.config/solana/id.json
Also, user should have correct Mint-authority JSON file

## Permission management for a single client/contract

mint_permission_token.sh operates with permission tokens balances of a signle client/contract. Script will estimate current state of client's/contract's permission tokens balances and mint selected tokens if needed. Here is common form of calling this script:
    > mint_permission_token.sh <solana_url> <evm_loader_id> <mint_authority_json_file> <allow|deny> <client|contract> <neon_eth_address>

Arguments:
    - solana_url - URL of Solana RPC endpoing (also, default replacements supported: mainnet-beta, testnet, devnet, localhost)
    - evm_loader_id - address of solana account where EVM is deployed to selected Solana network
    - mint_authority_json_file - path to JSON file on the local filesystem where private key to mint-authority stored
    - allow|deny - keyword determining what action should be performed on client's/contract's permissions
    - client|contract - keyword determining what type of account (client/contract) actually passed to script
    - neon_eth_address - NEON accoutn address (Eth-compatible - 0x0f45....)

## Permission management for several clients/contracts

mint_many.sh operates with permission tokens balances of several clients/contracts. Here is common form of calling this script:
    > mint_many.sh <solana_url> <evm_loader_id> <mint_authority_json_file> <allow|deny> <client|contract> <address_list_file>

This script accepts almost the same set of argument except the last one:
    - address_list_file - path to the file on a local filesystem containing list of NEON account addresses written one on a line (without any punktuation marks)

This script will call mint_permission_token.sh for every account from <address_list_file>
Failed account ids will be collected in output file with name <address_list_file>.err this file will have the same format as input file.