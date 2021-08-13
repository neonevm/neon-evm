#!/bin/bash
set -xeuo pipefail

echo "Deploy test..."
[ -e evm_loader-deploy_test-net ] || solana-keygen new --no-passphrase --force
ACCOUNT=$(solana address)

solana config set --url $SOLANA_URL
for i in {1..10}; do
    if solana cluster-version; then break; fi
    sleep 2
done

solana airdrop 5000
solana account $ACCOUNT

echo "Run tests for EVM Loader"

sed -i -e "s~SOLANA_URL~${SOLANA_URL}~g" test_token_config.yml
solana airdrop 5000 --config test_token_config.yml

# Create a token
export ETH_TOKEN_MINT=$(spl-token create-token --config test_token_config.yml -- test_token_keypair | grep -Po 'Creating token \K[^\n]*')
if [ ${#ETH_TOKEN_MINT} -eq 0 ]; then
  echo "ETH token mint is not created"
  exit 1
fi

TOKEN_ACCOUNT=$(spl-token --config test_token_config.yml create-account $ETH_TOKEN_MINT --owner $ACCOUNT | grep -Po 'Creating account \K[^\n]*')
spl-token --config test_token_config.yml mint $ETH_TOKEN_MINT 5000 -- $TOKEN_ACCOUNT
spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT

# Parse deployed contract address from output of solana-cli:
# Example output: `Program Id: 853qJy1Z8hfgHe194fVrYUbVDfx88ny7phSCHc481Fc6`
# EVM_LOADER will be empty if the match fails.
export EVM_LOADER=$(solana program deploy --upgrade-authority evm_loader-keypair.json evm_loader.so | sed '/Program Id: \([0-9A-Za-z]\+\)/,${s//\1/;b};s/^.*$//;$q1')
if [ ${#EVM_LOADER} -eq 0 ]; then
  echo  "EVM_LOADER is not deployed"
  exit 1
fi

sleep 25   # Wait while evm_loader deploy finalized

#generate collateral pool accounts
solana -k collateral-pool-keypair.json airdrop 1000
python3 collateral_pool_generator.py collateral-pool-keypair.json

python3 -m unittest discover -v -p 'test*.py'

echo "Deploy test success"
exit 0
