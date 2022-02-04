#!/bin/bash

SOLANA_URL=$1
EVM_LOADER=$2
MINT_AUTHORITY_FILE=$3
OPERATION=$4
ACCOUNT_TYPE=$5
ADDRESS_LIST_FILE=$6

show_help_and_exit() {
  echo "Usage: set_many_accts_permission.sh <solana_url> <evm_loader_id> <mint_authority_json_file> <allow|deny> <client|contract> <address_list_file>"
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

if [ -z "$ACCOUNT_TYPE" ]; then
  show_help_and_exit
fi

if [[ "$ACCOUNT_TYPE" != "client" && "$ACCOUNT_TYPE" != "contract" ]]; then
  echo "specify either 'client' or 'contract' account type as 5-th argument"
  exit 1 
fi

if [ -z "$ADDRESS_LIST_FILE" ]; then
  show_help_and_exit
fi

ERROR_FILE="$ADDRESS_LIST_FILE.err"
echo "Failed ID's will be collected in $ERROR_FILE"
touch $ERROR_FILE

while read line; do 
  ./set_single_acct_permission.sh $SOLANA_URL $EVM_LOADER $MINT_AUTHORITY_FILE $OPERATION $ACCOUNT_TYPE $line
  if [ "$?" -ne "0" ]; then
    echo "$line" >> $ERROR_FILE
  fi
done < $ADDRESS_LIST_FILE
