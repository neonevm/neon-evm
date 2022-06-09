import sys

from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed, Processed
from tests.solana_utils import OperatorAccount, account_with_seed, EVM_LOADER, solana_client, TransactionWithComputeBudget, send_transaction, create_account_with_seed, get_solana_balance

print("Run collateral_pool_generator.py")
wallet = OperatorAccount(sys.argv[1]).get_acc()
collateral_pool_base = wallet.public_key()
print(f"Collateral pool base: {collateral_pool_base}")
for collateral_pool_index in range(0, 10):
    COLLATERAL_SEED_PREFIX = "collateral_seed_"
    seed = COLLATERAL_SEED_PREFIX + str(collateral_pool_index)
    collateral_pool_address = account_with_seed(PublicKey(collateral_pool_base), seed, PublicKey(EVM_LOADER))
    print("Collateral pool address: ", collateral_pool_address)
    if get_solana_balance(collateral_pool_address) == 0:
        print("Create it")
        minimum_balance = solana_client.get_minimum_balance_for_rent_exemption(0, commitment=Confirmed)["result"]
        trx = TransactionWithComputeBudget()
        trx.add(create_account_with_seed(wallet.public_key(), PublicKey(collateral_pool_base), seed, minimum_balance, 0, PublicKey(EVM_LOADER)))
        result = send_transaction(solana_client, trx, Keypair.from_secret_key(wallet.secret_key()), Processed)
        print(result)
print(collateral_pool_base)
