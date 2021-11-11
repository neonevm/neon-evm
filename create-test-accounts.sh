#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set -u "$SOLANA_URL"

if [ -z "$1" ]; then
  echo "number of account is required. Usage:"
  echo "     create-test-accounts.sh <solana_url> <num_accounts>"
  exit 2
fi

NUM_ACCOUNTS=$1

for i in {1..$NUM_ACCOUNTS}; do
  echo "Creating test account-$i"
  ID_FILE="$HOME/.config/solana/id"
  if [ "$i" -gt "1" ]; then
    ID_FILE="${ID_FILE}${i}.json"
  else
    ID_FILE="${ID_FILE}.json"
  fi

  echo "ID file is $ID_FILE"
  if [ ! -f "$ID_FILE" ]; then
    echo "Creating new wallet"
    solana-keygen new --no-passphrase -o "$ID_FILE"
  fi
  ACCOUNT=$(solana address -k "$ID_FILE")
  echo "New account $ACCOUNT"
  if ! solana account "$ACCOUNT"; then
    echo "airdropping..."
    solana airdrop 5000 "$ACCOUNT"
    # check that balance >= 10 otherwise airdroping by 1 SOL up to 10
    BALANCE=$(solana balance -k "$ACCOUNT" | tr '.' '\t'| tr '[:space:]' '\t' | cut -f1)
    while [ "$BALANCE" -lt 10 ]; do
      solana airdrop 1 "$ACCOUNT"
      sleep 1
      BALANCE=$(solana balance -k "$ACCOUNT" | tr '.' '\t'| tr '[:space:]' '\t' | cut -f1)
    done
  fi
done


