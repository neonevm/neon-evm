#!/bin/bash

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set -u "$SOLANA_URL"

WAIT_TIME=0
if [ -z "$1" ]; then
  if solana cluster-version >/dev/null 2>&1; then exit 0; fi
  echo "unable to connect to solana cluster $SOLANA_URL"
  exit 1
fi

WAIT_TIME=$1
echo "Waiting $WAIT_TIME seconds for solana cluster to be available at $SOLANA_URL"
for i in $(seq 1 $WAIT_TIME); do
    if solana cluster-version >/dev/null 2>&1; then exit 0; fi
    sleep 1
done

echo "unable to connect to solana cluster $SOLANA_URL"
exit 1


