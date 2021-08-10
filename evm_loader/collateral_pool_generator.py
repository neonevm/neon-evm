from solana.publickey import PublicKey
from solana.transaction import Transaction
import sys
from solana_utils import *


wallet = WalletAccount(wallet_path())
creator_acc = client.get_account_info(PublicKey(EVM_LOADER))['value']['owner']
for collateral_pool_index in range(0, 10):
    COLLATERAL_SEED_PREFIX = "collateral_seed_"
    seed = COLLATERAL_SEED_PREFIX + str(collateral_pool_index)
    collateral_pool_address = accountWithSeed(PublicKey(creator_acc), seed, PublicKey(EVM_LOADER))
    print("Collateral pool address: ", collateral_pool_address)
    if getBalance(collateral_pool_address) == 0:
        print("Creating...")
        minimum_balance = client.get_minimum_balance_for_rent_exemption(0, commitment=Confirmed)["result"]
        trx = Transaction()
        trx.add(createAccountWithSeed(wallet.public_key(), PublicKey(creator_acc), seed, minimum_balance, 0, PublicKey(EVM_LOADER)))
        result = send_transaction(client, trx, wallet)
        print(result)
