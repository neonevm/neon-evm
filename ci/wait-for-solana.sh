#!/bin/bash
set -euo pipefail

function check_solana() {
  local DATA='{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
  local RESULT='"ok"'
  local CHECK_COMMAND="curl http://localhost:8899 -s -X POST -H 'Content-Type: application/json' -d '$DATA' | grep -cF '$RESULT'"

  local CHECK_COMMAND_RESULT=$(eval $CHECK_COMMAND)
  if [[ "$CHECK_COMMAND_RESULT" == "1" ]]; then
    exit 0
  fi
  exit 1
}

if [ $# -eq 0 ]; then
  if $(check_solana); then exit 0; fi
else
  WAIT_TIME=$1
  echo "Waiting $WAIT_TIME seconds for solana cluster to be available at localhost"
  for i in $(seq 1 $WAIT_TIME); do
    echo "Try solana getHealth count=$i"
    if $(check_solana); then
      echo "Executed solana getHealth successfully after count=$i"
      exit 0
    fi
    sleep 1
  done
fi

echo "unable to connect to solana cluster localhost"
exit 1
