echo "Deploying Solidity contracts..."

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

# NOTE: If you change this key, keep in mind to update also token addresses in solidity/tokenlist.json file
export DEPLOYER_PRIVATE_KEY='0x4deacb079b4714c38f39508aa8900039f2721ed8686835d43347ba9267da767b'
export DEPLOYER_PUBLIC_KEY=$(python3 get_deployer_address.py)
echo "Deployer public key: $DEPLOYER_PUBLIC_KEY"

export EVM_LOADER=$(solana address -k /opt/evm_loader-keypair.json)
echo "EVM address: $EVM_LOADER"
echo "Creating deployer account $DEPLOYER_PUBLIC_KEY"
neon-cli --evm_loader "$EVM_LOADER" --url "$SOLANA_URL" create-ether-account "$DEPLOYER_PUBLIC_KEY"

echo "Depositing NEONs to deployer $DEPLOYER_PUBLIC_KEY"
export ETH_TOKEN_MINT=$(solana address -k /opt/neon_token_keypair.json)
ACCOUNT=$(solana address --keypair /root/.config/solana/id.json)
TOKEN_ACCOUNT=$(spl-token create-account $ETH_TOKEN_MINT --owner $ACCOUNT | grep -Po 'Creating account \K[^\n]*')
spl-token mint $ETH_TOKEN_MINT 5000 --owner evm_loader-keypair.json -- $TOKEN_ACCOUNT
spl-token balance $ETH_TOKEN_MINT --owner $ACCOUNT
neon-cli --evm_loader "$EVM_LOADER" --url "$SOLANA_URL" deposit 1000 "$DEPLOYER_PUBLIC_KEY"

echo "Compiling and deploying contracts"
cd /opt/contracts/
npx hardhat compile
npx hardhat run --network ci /opt/contracts/scripts/deploy.js