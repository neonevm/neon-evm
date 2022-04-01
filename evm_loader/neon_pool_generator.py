from solana.publickey import PublicKey
from solana.transaction import Transaction
from solana_utils import *
from solana.rpc.commitment import Confirmed
import sys

evm_loader = PublicKey(sys.argv[1])
mint = PublicKey(sys.argv[2])

signer = OperatorAccount().get_acc()
authority_account = PublicKey.find_program_address([b"Deposit"], evm_loader)[0]
pool = get_associated_token_address(authority_account, mint)
print("Pool: ", pool)

pool_account_exists = client.get_account_info(pool, commitment="processed")["result"]["value"] is not None
if pool_account_exists:
    print("Pool account already exists")
    exit(0)

trx = TransactionWithComputeBudget()
trx.add(create_associated_token_account(signer.public_key(), authority_account, mint))
result = send_transaction(client, trx, signer)
print(result)
