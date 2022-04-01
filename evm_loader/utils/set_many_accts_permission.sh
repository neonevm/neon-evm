#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL not defined"
  exit 1
fi

if [ -z "$EVM_LOADER" ]; then
  echo "EVM_LOADER not defined"
  exit 1
fi

if [ -z "$MINT_AUTHORITY_FILE" ]; then
  echo "MINT_AUTHORITY_FILE not defined"
  exit 1
fi

if [ -z "$OPERATION" ]; then
  echo "OPERATION not defined"
  exit 1
fi

if [[ "$OPERATION" != "allow" && "$OPERATION" != "deny" ]]; then
  echo "specify either 'allow' or 'deny' operation as 4-th argument"
  exit 1 
fi

if [ -z "$ACCOUNT_TYPE" ]; then
  echo "ACCOUNT_TYPE not defined"
  exit 1
fi

if [[ "$ACCOUNT_TYPE" != "client" && "$ACCOUNT_TYPE" != "contract" ]]; then
  echo "specify either 'client' or 'contract' account type as 5-th argument"
  exit 1 
fi

if [ -z "$ADDRESS_LIST_FILE" ]; then
  echo "ADDRESS_LIST_FILE not defined"
  exit 1
fi

ERROR_FILE="$ADDRESS_LIST_FILE.err"
echo "Failed ID's will be collected in $ERROR_FILE"
touch $ERROR_FILE

while read line; do
  export NEON_ETH_ADDRESS=$line
  ./set_single_acct_permission.sh
  if [ "$?" -ne "0" ]; then
    echo "$line" >> $ERROR_FILE
  fi
done < $ADDRESS_LIST_FILE
