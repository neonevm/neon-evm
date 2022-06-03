#!/bin/bash

COMPONENT="${2:-Undefined}"
FILENAME=$(basename "$0")

echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} Start creating test accounts"

if [ -z "${SOLANA_URL}" ]; then
  echo "$(date "+%F %X.%3N") E ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} SOLANA_URL is not set"
  exit 1
fi

solana config set -u "${SOLANA_URL}"

if [ -z "${1}" ]; then
  echo "$(date "+%F %X.%3N") E ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} Error: number of accounts is required. Usage: create-test-accounts.sh <num_accounts>"
  exit 2
fi

function createAccount() {
  declare i=${1}
  echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} Creating test account-$i"
    ID_FILE="$HOME/.config/solana/id"
    if [ "${i}" -gt "1" ]; then
      ID_FILE="${ID_FILE}${i}.json"
    else
      ID_FILE="${ID_FILE}.json"
    fi

    echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} ID file is ${ID_FILE}"
    if [ ! -f "${ID_FILE}" ]; then
      echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} Creating new wallet"
      solana-keygen new --no-passphrase -o "${ID_FILE}"
    fi
    ACCOUNT=$(solana address -k "${ID_FILE}")
    echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} New account ${ACCOUNT}"
    if ! solana account "${ACCOUNT}"; then
      echo "$(date "+%F %X.%3N") I ${FILENAME}:${LINENO} $$ ${COMPONENT}:CreateTestAcc {} airdropping..."
      solana airdrop 5000 "${ACCOUNT}"
      # check that balance >= 10 otherwise airdroping by 1 SOL up to 10
      BALANCE=$(solana balance "${ACCOUNT}" | tr '.' '\t'| tr '[:space:]' '\t' | cut -f1)
      while [ "${BALANCE}" -lt 10 ]; do
        solana airdrop 1 "${ACCOUNT}"
        sleep 1
        BALANCE=$(solana balance "${ACCOUNT}" | tr '.' '\t'| tr '[:space:]' '\t' | cut -f1)
      done
    fi
}

NUM_ACCOUNTS=${1}
createAccount 1
for i in $(seq 2 ${NUM_ACCOUNTS}); do
  createAccount ${i} &
done
