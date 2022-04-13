import os
from web3 import Account

print(f"{Account.from_key(os.environ['DEPLOYER_PRIVATE_KEY']).address}")
