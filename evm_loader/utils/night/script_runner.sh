#!/bin/sh
# param 1 : allow|deny
# param 2 : client|contract


# neon_revision="cf20aac797114fc2741b92a5b0cac424637d68f5"
solana_url=http://proxy.night.stand.neontest.xyz/solana
evm_loader_id=eeLSJgWzzxrqKv1UxtRVVH8FX3qCQWUs9QuAjJpETGU
mint_authority_json_file=../evm_loader_keypair.json
permission="\"allow\"" # "deny"
grantee="client" # "contract"
# whitelist 01
neon_eth_address=0x4cEf46ef9064a6Ec7FfB9a6C905845dc345bfd12
# whitelist 02
# neon_eth_address=""
# whitelist 02
# neon_eth_address=""
address_list_file=./addresses.txt
echo "./set_single_acct_permission.sh $solana_url $evm_loader_id $mint_authority_json_file $1 $2 $neon_eth_address"

../set_single_acct_permission.sh "$solana_url" "$evm_loader_id" "$mint_authority_json_file" $1 $2 "$neon_eth_address"
