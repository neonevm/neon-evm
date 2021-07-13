#! /bin/bash -p

# args:
#   $1 - count of processes
#   $2 - count of items (accounts, contracts, transactions, ...)
#   $3 - tcp | udp
#
# example:
#   run.sh 10 10 tcp  

echo -e '\nCOUNT OF PROCESSES' $1
echo COUNT OF ITEMS $2

echo -e '\nDEPLOY'
for i in $(seq $1); do python3 run.py --step deploy --count $2 --postfix $i & done    
P=$!
wait $P 

echo -e '\nCREATE SENDERS'
for i in $(seq $1); do python3 run.py --step create_senders --count $2 --postfix $i & done    
P=$!
wait $P 

echo -e '\nCREATE ACCOUNTS'
for i in $(seq $1); do python3 run.py --step create_acc --count $2 --scheme one-to-one --postfix $i & done    
P=$!
wait $P 

echo -e '\nCREATE TRANSACTIONS'
for i in $(seq $1)
do 
    python3 run.py --step create_trx --count $2 --scheme one-to-one --postfix $i 
done
P=$!
wait $P 

echo -e 'SEND TRANSACTIONS'
for i in $(seq $1); 
do 
    ./sender --url $SOLANA_URL --evm_loader $EVM_LOADER transaction.json$i sender.json$i verify.json$i --client $3 & 
done    
P=$!
wait $P

echo -e '\nVERIFY TRANSACTIONS'
for i in $(seq $1)
do 
    python3 run.py --step verify_trx --count $2  --postfix $i 
done
P=$!
wait $P 

