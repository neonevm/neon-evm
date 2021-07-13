#!/bin/bash
set -xeuo pipefail
export SOLANA_URL=http://localhost:8899

echo "Deploy performance test..."
[ -e evm_loader-deploy_test-net ] || solana-keygen new --no-passphrase --force
ACCOUNT=$(solana address)

solana config set --url $SOLANA_URL
for i in {1..10}; do
    if solana cluster-version; then break; fi
    sleep 2
done

solana airdrop 1000
solana account $ACCOUNT

echo "deploy EVM Loader"

export EVM_LOADER=$(solana deploy evm_loader.so | sed '/Program Id: \([0-9A-Za-z]\+\)/,${s//\1/;b};s/^.*$//;$q1')
if [ ${#EVM_LOADER} -eq 0 ]; then
  echo  "EVM_LOADER is not deployed"
  exit 1
fi
sleep 25   # Wait while evm_loader deploy finalized


echo -e /nEVM_LOADER $EVM_LOADER
echo -e "run script run.sh to start performance test"
echo "before starting, set EVM_LOADER environment variable"
echo "args desc:  ./run.sh <count of processes> <count of itmes> tcp|udp"
echo -e "/nexample: export EVM_LOADER=9tPwQFA392rAYYqoy4wkX847PopT73J2Fyppoxe7Rmg2 &   run.sh 10 10 tcp"

exit 0
