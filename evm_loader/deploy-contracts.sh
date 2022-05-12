echo "Deploying Solidity contracts..."

if [ -z "$SOLANA_URL" ]; then
  echo "SOLANA_URL is not set"
  exit 1
fi

solana config set --url "$SOLANA_URL"

# NOTE: If you change this key, keep in mind to update also token addresses in solidity/tokenlist.json file
export DEPLOYER_PRIVATE_KEY='0x4deacb079b4714c38f39508aa8900039f2721ed8686835d43347ba9267da767b'
export DEPLOYER_PUBLIC_KEY=$(python3 get_deployer_address.py)
echo "Deployer public key: $DEPLOYER_PUBLIC_KEY"

export EVM_LOADER=$(solana address -k /opt/evm_loader-keypair.json)
echo "EVM address: $EVM_LOADER"
echo "Creating deployer account $DEPLOYER_PUBLIC_KEY"
neon-cli --evm_loader "$EVM_LOADER" --url "$SOLANA_URL" create-ether-account "$DEPLOYER_PUBLIC_KEY"

function mint()
{
    token_name=$1
    token_mint=$2
    echo
    echo "Depositing ${token_name}s to deployer $DEPLOYER_PUBLIC_KEY"
    ACCOUNT=$(solana address --keypair /root/.config/solana/id.json)
    echo "Solana account $ACCOUNT"
    TOKEN_ACCOUNT=$(spl-token create-account $token_mint --owner $ACCOUNT | grep -Po 'Creating account \K[^\n]*')
    echo "Token accout $TOKEN_ACCOUNT"
    spl-token mint $token_mint 5000 --owner /opt/evm_loader-keypair.json -- $TOKEN_ACCOUNT
    echo "Balance of $ACCOUNT is: $(spl-token balance $token_mint --owner $ACCOUNT) ${token_name}s"
}

mint "NEON" "$(solana address -k /opt/neon_token_keypair.json)"

neon-cli --commitment=processed --url "$SOLANA_URL" deposit 1000000000000 "$DEPLOYER_PUBLIC_KEY" --evm_loader "$EVM_LOADER"

echo "Compiling and deploying contracts"
cd /opt/contracts/
npx hardhat compile
sleep 20
npx hardhat run --network ci /opt/contracts/scripts/deploy.js

mint "USDT" "B77GCLJPHQAzH5dMfeCMWeaKV4zzWV2WibaAYrscxe4L"
