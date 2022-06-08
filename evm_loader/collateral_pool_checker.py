import sys
from solana.publickey import PublicKey
from solana_utils import OperatorAccount, account_with_seed, EVM_LOADER, get_solana_balance


wallet = OperatorAccount(sys.argv[1]).get_acc()
collateral_pool_base = wallet.public_key()
print(collateral_pool_base)

COLLATERAL_MAX_INDEX = 10
COLLATERAL_SEED_PREFIX = "collateral_seed_"
seed = COLLATERAL_SEED_PREFIX + str(COLLATERAL_MAX_INDEX - 1)
collateral_pool_address = account_with_seed(PublicKey(collateral_pool_base), seed, PublicKey(EVM_LOADER))
print("Collateral pool address: ", collateral_pool_address)
if get_solana_balance(collateral_pool_address) != 0:
    exit(0)
exit(1)
