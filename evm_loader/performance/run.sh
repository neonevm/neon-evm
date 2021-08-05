#! /bin/bash -p
# before start set EVM_LOADER environment variable
#
# args:
#   $1 - count of processes
#   $2 - count of items (accounts, contracts, transactions, ...)
#   $3 - tcp | udp
#
# example:
#   run.sh 10 10 tcp  

if [ ${#EVM_LOADER} -eq 0 ]; then
  echo  "EVM_LOADER is not deployed"
  exit 1
fi

echo EVM_LOADER $EVM_LOADER

echo -e '\nCOUNT OF PROCESSES' $1
echo COUNT OF ITEMS $2

echo DEPLOY
parallel --jobs 0 --keep-order --results log.deploy python3 run.py --step deploy --count $2  --postfix {}  :::  $(seq $1)
echo CREATE_SENDERS
parallel --jobs 0 --keep-order --results log.create_senders python3 run.py --step create_senders --count $2  --postfix {}  :::  $(seq $1)
echo CREATE_ACCOUNTS
parallel --jobs 0 --keep-order --results log.create_acc  python3 run.py --step create_acc --count $2 --scheme one-to-one --postfix {}  :::  $(seq $1)
echo CREATE_TRANSACTIONS
parallel --jobs 0 --keep-order --results log.create_trx  python3 run.py --step create_trx --count $2 --postfix {}  :::  $(seq $1)
echo SEND_TRANSACTIONS
parallel --jobs 0 --keep-order --results log.send_trx ./sender --url $SOLANA_URL --evm_loader $EVM_LOADER transaction.json{} sender.json{} verify.json{} --client $3 ::: $(seq $1)
echo VERIFY_TRANSACTIONS
parallel --jobs 0 --keep-order --results log.verify_trx  python3 run.py --step verify_trx  --postfix {}  :::  $(seq $1)
