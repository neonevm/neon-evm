#!/bin/bash

SOLANA_URL=$1
EVM_LOADER=$2
MINT_AUTHORITY_FILE=$3
OPERATION=$4
NEON_ETH_ADDRESS=$5

show_help_and_exit() {
  echo "Usage: mint_permission_token.sh <solana_url> <evm_loader_id> <mint_authority_json_file> <allow|deny> <neon_eth_address>"
  exit 1
}

if [ -z "$SOLANA_URL" ]; then
  show_help_and_exit
fi

if [ -z "$EVM_LOADER" ]; then
  show_help_and_exit
fi

if [ -z "$MINT_AUTHORITY_FILE" ]; then
  show_help_and_exit
fi

if [ -z "$OPERATION" ]; then
  show_help_and_exit
fi

if [[ "$OPERATION" != "allow" && "$OPERATION" != "deny" ]]; then
  echo "specify either 'allow' or 'deny' operation as 4-th argument"
  exit 1 
fi

if [ -z "$NEON_ETH_ADDRESS" ]; then
  show_help_and_exit
fi

export $(neon-cli --commitment confirmed --url $SOLANA_URL --evm_loader="$EVM_LOADER" neon-elf-params)
if [ "$?" -ne "0" ]; then
  exit 1
fi

echo "" #Just to separate different accounts in script output
echo "Neon permission allowance token address: $NEON_PERMISSION_ALLOWANCE_TOKEN"
echo "Neon permission denial token address: $NEON_PERMISSION_DENIAL_TOKEN"
echo "Minimal client allowance balance: $NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE"
echo "Minimal contract allowance balance: $NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE"


get_or_create_token_account() {
   OWNER=$1
   TOKEN_MINT=$2
   TOKEN_ACCOUNT=$( (spl-token create-account --url "$SOLANA_URL" --owner "$OWNER" "$TOKEN_MINT" || true) | grep -Po 'Creating account \K[^\n]*')
   if [ "$?" -ne "0" ]; then
      return "$?"
   fi
   
   echo $TOKEN_ACCOUNT
   return 0
}

get_token_balance() {
  TOKEN_ACCOUNT=$1
  TOKEN_BALANCE=$(spl-token balance --url "$SOLANA_URL" --address "$TOKEN_ACCOUNT")
  if [ "$?" -ne "0" ]; then
    echo "Failed to read token account balance: $TOKEN_ACCOUNT"
    return 1
  fi
  
  echo "$TOKEN_BALANCE"
  return 0
}

calc_permission_tokens_diff() { 
  NEON_ADDRESS=$1  
  NEON_SOLANA_ADDRESS=$(neon-cli create-program-address --evm_loader "$EVM_LOADER" "$NEON_ADDRESS" | awk '{ print $1 }')
  echo "Processing NEON account $NEON_ADDRESS <--> $NEON_SOLANA_ADDRESS"  
  
  ALLOWANCE_TOKEN_ACCOUNT=$(get_or_create_token_account $NEON_SOLANA_ADDRESS $NEON_PERMISSION_ALLOWANCE_TOKEN)
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  DENIAL_TOKEN_ACCOUNT=$(get_or_create_token_account $NEON_SOLANA_ADDRESS $NEON_PERMISSION_DENIAL_TOKEN)
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  ALLOWANCE_TOKEN_BALANCE=$(get_token_balance $ALLOWANCE_TOKEN_ACCOUNT)
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  DENIAL_TOKEN_BALANCE=$(get_token_balance $DENIAL_TOKEN_ACCOUNT)
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  echo "Allowance token account $ALLOWANCE_TOKEN_ACCOUNT balance: $ALLOWANCE_TOKEN_BALANCE"
  echo "Denial token account $DENIAL_TOKEN_ACCOUNT balance: $DENIAL_TOKEN_BALANCE"
  export DIFFERENCE=$(($ALLOWANCE_TOKEN_BALANCE - $DENIAL_TOKEN_BALANCE))
}

mint_denial_token() {
  NEON_ADDRESS=$1
  calc_permission_tokens_diff $NEON_ADDRESS
  
  if [ "$DIFFERENCE" -ge "$NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE" ]; then
    MINT_AMOUNT=$(($DIFFERENCE - $NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE + 1))
    echo "Minting $MINT_AMOUNT denial tokens to $NEON_ADDRESS"
    spl-token mint --url http://localhost:8899 --mint-authority "$MINT_AUTHORITY_FILE" "$NEON_PERMISSION_DENIAL_TOKEN" "$MINT_AMOUNT" -- "$DENIAL_TOKEN_ACCOUNT"
  else
    echo "There's no need to mint denial token"
  fi
}

mint_allowance_token() {
  NEON_ADDRESS=$1
  calc_permission_tokens_diff $NEON_ADDRESS

  if [ "$DIFFERENCE" -lt "$NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE" ]; then
    MINT_AMOUNT=$(($NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE - $DIFFERENCE))
    echo "Minting $MINT_AMOUNT allowance tokens to $NEON_ADDRESS"
    spl-token mint --url http://localhost:8899 --mint-authority "$MINT_AUTHORITY_FILE" "$NEON_PERMISSION_ALLOWANCE_TOKEN" "$MINT_AMOUNT" -- "$ALLOWANCE_TOKEN_ACCOUNT"
  else
    echo "There's no need to mint allowance token"
  fi
}

if [ "$OPERATION" == "allow" ]; then
  mint_allowance_token "$NEON_ETH_ADDRESS"
  exit "$?"
fi

if [ "$OPERATION" == "deny" ]; then
  mint_denial_token "$NEON_ETH_ADDRESS"
  exit "$?"
fi

