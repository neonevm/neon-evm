#! /bin/bash -p
#
# Infinite sending transactions
# Is performed spl-token transfers by the evm_loader instruction 05
#
# before start set EVM_LOADER and SOLANA_URL environment variables
#
# args:
#   $1 - count of processes
#   $2 - tcp | udp
#   $3 - sender/sender.json
#   $4 - verify/verify.json
#   $5 - collateral/collateral.json
#   $6 - account/account.json
#
# example:
# ./send_unlimit.sh 4 tcp 5000/sender/sender.json 5000/verify/verify.json 5000/collateral/collateral.json 5000/account/account.json



if [ ${#EVM_LOADER} -eq 0 ]; then
  echo  "EVM_LOADER is not deployed"
  exit 1
fi

if [ ${#SOLANA_URL} -eq 0 ]; then
  echo  "SOLANA_URL is not defined"
  exit 1
fi

echo EVM_LOADER $EVM_LOADER
echo SOLANA_URL $SOLANA_URL
echo -e '\nCOUNT OF PROCESSES' $1

echo senders: $3
echo verify: $4
echo collaterals: $5
echo accounts :$6

parallel --jobs 0 --keep-order --results log.send ./send_unlimit   $3{} $4{} $5{} $6{}  :::  $(seq $1)
