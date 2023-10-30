#!/bin/bash
set -euo pipefail

: ${SOLANA_URL:?is not set}

if [ $# -eq 0 ]; then
  if solana -u $SOLANA_URL cluster-version >/dev/null 2>&1; then exit 0; fi
else
  WAIT_TIME=$1
  echo "Waiting $WAIT_TIME seconds for solana cluster to be available at $SOLANA_URL"
  for i in $(seq 1 $WAIT_TIME); do
      echo "Try solana cluster-version count=$i"
      if solana -u $SOLANA_URL cluster-version; then
        echo "Executed solana cluster-version successfully after count=$i"
        exit 0
      fi
      sleep 1
  done
fi

echo "unable to connect to solana cluster $SOLANA_URL"
exit 1
