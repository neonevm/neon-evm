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
#   $3 - delay in microceconds
#   $4 - sender/sender.json
#   $5 - verify/verify.json
#   $6 - collateral/collateral.json
#   $7 - account/account.json
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

echo senders: $4
echo verify: $5
echo collaterals: $6
echo accounts :$7

parallel --jobs 0 --keep-order --results log.send ./send_unlimit --client $2 --delay $3 $4{} $5{} $6{} $7{}  :::  $(seq $1)
