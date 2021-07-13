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


for i in $(seq $1)
do
    echo $i
    python3 run.py --step deploy --count $2 --postfix $i &
done

exit 0
