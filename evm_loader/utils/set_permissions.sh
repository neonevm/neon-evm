#!/bin/bash

export CONFIG_NAME=$1
SINGLE_OR_MANY=$2
export ACCOUNT_TYPE=$3
export OPERATION=$4

show_usage() {
  echo "usage: ./run_config.sh <CONFIG_NAME> single|many <ACCOUNT_TYPE> <OPERATION> [NEON_ETH_ADDR]"
  echo "    CONFIG_NAME - name of the stand configuration - must correspond to subdirectory in ./config"
  echo "    ACCOUNT_TYPE - either 'contract' or 'client'"
  echo "    OPERATION - either 'allow' or 'deny'"
  echo "    NEON_ETH_ADDR - ONLY FOR CASE WHEN 'single' set to second argument - ETH-like address of account"
}

if [ -z "$CONFIG_NAME" ]; then
  echo "CONFIG_NAME not set"
  show_usage
  exit 1
fi

if [ -z "$SINGLE_OR_MANY" ]; then
  echo "CONFIG_NAME not set"
  show_usage
  exit 1
fi

if [ -z "$ACCOUNT_TYPE" ]; then
  echo "ACCOUNT_TYPE not set"
  show_usage
  exit 1
fi

if [ -z "$OPERATION" ]; then
  echo "OPERATION not set"
  show_usage
  exit 1
fi

if [ "$SINGLE_OR_MANY" == "single" ]; then
  if [ -z "$5" ]; then
    echo "NEON_ETH_ADDRESS not set"
    show_usage
    exit 1
  fi
  export NEON_ETH_ADDRESS=$5
  docker-compose up set_single_acct_permission

elif [ "$SINGLE_OR_MANY" == "many" ]; then
  docker-compose up set_many_accts_permission

else
  echo "unknown argument '$SINGLE_OR_MANY'"
  echo "usage: ./run_config.sh <CONFIG_NAME> single|many"
  exit 1
fi



