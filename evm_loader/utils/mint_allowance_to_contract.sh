#!/bin/bash

SOLANA_URL=$1
EVM_LOADER=$2
MINT_AUTHORITY_FILE=$3

show_help_and_exit() {
  echo "Usage: mint_allowance_to_contract.sh <solana_url> <evm_loader_id> <mint_authority_json_file>:"
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

export $(neon-cli --commitment confirmed --url $SOLANA_URL --evm_loader="$EVM_LOADER" neon-elf-params)

echo "Neon permission allowance token address: $NEON_PERMISSION_ALLOWANCE_TOKEN"
echo "Neon permission denial token address: $NEON_PERMISSION_DENIAL_TOKEN"
echo "Minimal client allowance balance: $NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE"
echo "Minimal contract allowance balance: $NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE"


get_token_account_address() {
  OWNER=$1
  TOKEN_MINT=$2
  TOKEN_ACCOUNT_ADDRESS=$( (spl-token --url "$SOLANA_URL" address --owner "$OWNER" --token "$TOKEN_MINT" --verbose || true) | grep -Po 'Associated token address: \K[^\n]*')
  if [ "$?" -eq "0" ]; then
    echo "$TOKEN_ACCOUNT_ADDRESS"
    return 0
  fi
  
  echo "Failed to compute associated token account"
  return 1 
}

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


# Helper function producing TOKEN_BALANCE variable (or returning non-zero value in case of error)
get_token_balance() {
  OWNER=$1
  TOKEN_MINT=$2
 
  TOKEN_ACCOUNT=$( (spl-token create-account --url "$SOLANA_URL" --owner "$OWNER" "$TOKEN_MINT" || true) | grep -Po 'Creating account \K[^\n]*')
  TOKEN_BALANCE=$(spl-token balance --url "$SOLANA_URL" --owner "$OWNER" "$NEON_PERMISSION_ALLOWANCE_TOKEN")
  if [ "$?" -ne "0" ]; then
    return "$?"
  fi
  
  return 0
}

# Helper function producing DIFFERENCE variable (or returning non-zero value in case of error)
estimate_diff() {
  OWNER=$1

  get_token_balance "$OWNER" "$NEON_PERMISSION_ALLOWANCE_TOKEN"
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  ALLOWANCE_TOKEN_BALANCE=$TOKEN_BALANCE
  
  get_token_balance "$OWNER" "$NEON_PERMISSION_DENIAL_TOKEN"
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  DENIAL_TOKEN_BALANCE=$TOKEN_BALANCE
  
  echo "Allowance token balance $ALLOWANCE_TOKEN_BALANCE"
  echo "Denial token balance $DENIAL_TOKEN_BALANCE"
  DIFFERENCE=$(($ALLOWANCE_TOKEN_BALANCE - $DENIAL_TOKEN_BALANCE))
  echo "Difference $DIFFERENCE"
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

# Helper function producing DIFFERENCE variable (or returning non-zero value in case of error)
estimate_diff2() {
  ALLOWANCE_TOKEN_ACCOUNT=$1
  DENIAL_TOKEN_ACCOUNT=$2
  
  ALLOWANCE_TOKEN_BALANCE=$(spl-token balance --url "$SOLANA_URL" --address "$ALLOWANCE_TOKEN_ACCOUNT")
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  DENIAL_TOKEN_BALANCE=$(spl-token balance --url "$SOLANA_URL" --address "$DENIAL_TOKEN_ACCOUNT")
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  echo "Allowance token balance $ALLOWANCE_TOKEN_BALANCE"
  echo "Denial token balance $DENIAL_TOKEN_BALANCE"
  DIFFERENCE=$(($ALLOWANCE_TOKEN_BALANCE - $DENIAL_TOKEN_BALANCE))
  echo "$DIFFERENCE"
  return 0
}

mint_allowance_token() {
  NEON_ADDRESS=$1
  NEON_SOLANA_ADDRESS=$(neon-cli create-program-address --evm_loader "$EVM_LOADER" "$NEON_ADDRESS" | awk '{ print $1 }')                          
  echo ""
  echo "Processing NEON account $NEON_ADDRESS ($NEON_SOLANA_ADDRESS)"
  
  estimate_diff $NEON_SOLANA_ADDRESS
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  if [ "$DIFFERENCE" -lt "$NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE" ]; then
    MINT_AMOUNT=$(($NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE - $DIFFERENCE))
    echo "Minting $MINT_AMOUNT allowance tokens to $NEON_ADDRESS"
    spl-token mint --url http://localhost:8899 --mint-authority "$MINT_AUTHORITY_FILE" "$NEON_SOLANA_ADDRESS" "$MINT_AMOUNT"
  fi
}


mint_denial_token() {
  NEON_ADDRESS=$1
  NEON_SOLANA_ADDRESS=$(neon-cli create-program-address --evm_loader "$EVM_LOADER" "$NEON_ADDRESS" | awk '{ print $1 }')                          
  echo ""
  echo "Processing NEON account $NEON_ADDRESS ($NEON_SOLANA_ADDRESS)"
  
  estimate_diff $NEON_SOLANA_ADDRESS
  if [ "$?" -ne "0" ]; then
    return 1
  fi
  
  if [ "$DIFFERENCE" -ge "$NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE" ]; then
    MINT_AMOUNT=$(($DIFFERENCE - $NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE + 1))
    echo "Minting $MINT_AMOUNT denial tokens to $NEON_ADDRESS"
    spl-token mint --url http://localhost:8899 --mint-authority "$MINT_AUTHORITY_FILE" "$NEON_SOLANA_ADDRESS" "$MINT_AMOUNT"
  fi
}

mint_denial_token2() {
  echo "" #Just to separate different accounts in script output

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
  DIFFERENCE=$(($ALLOWANCE_TOKEN_BALANCE - $DENIAL_TOKEN_BALANCE))
  
  if [ "$DIFFERENCE" -ge "$NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE" ]; then
    MINT_AMOUNT=$(($DIFFERENCE - $NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE + 1))
    echo "Minting $MINT_AMOUNT denial tokens to $NEON_ADDRESS"
    spl-token mint --url http://localhost:8899 --mint-authority "$MINT_AUTHORITY_FILE" "$NEON_PERMISSION_DENIAL_TOKEN" "$MINT_AMOUNT" -- "$DENIAL_TOKEN_ACCOUNT"
  else
    echo "There's no need to mint denial token"
  fi
}

TEST_ADDRESS="0x6926a674a132747fb7F28F34Dab0B3861Ff503e6"
#mint_denial_token "$TEST_ADDRESS"
mint_denial_token2 "$TEST_ADDRESS"
