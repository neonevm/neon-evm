from solana.publickey import PublicKey
from solana.transaction import Transaction
from solana_utils import *
import sys

wallet = WalletAccount(sys.argv[1]).get_acc()
collateral_pool_base = wallet.public_key()
print(collateral_pool_base)
for collateral_pool_index in range(0, 10):
    COLLATERAL_SEED_PREFIX = "collateral_seed_"
    seed = COLLATERAL_SEED_PREFIX + str(collateral_pool_index)
    collateral_pool_address = accountWithSeed(PublicKey(collateral_pool_base), seed, PublicKey(EVM_LOADER))
    print("Collateral pool address: ", collateral_pool_address)
    if getBalance(collateral_pool_address) == 0:
        print("Creating...")
        minimum_balance = client.get_minimum_balance_for_rent_exemption(0, commitment=Confirmed)["result"]
        trx = Transaction()
        trx.add(createAccountWithSeed(wallet.public_key(), PublicKey(collateral_pool_base), seed, minimum_balance, 0, PublicKey(EVM_LOADER)))
        result = send_transaction(client, trx, wallet)
        print(result)
print(collateral_pool_base)
