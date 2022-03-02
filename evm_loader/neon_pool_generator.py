from solana.publickey import PublicKey
from solana.transaction import Transaction
from solana_utils import *
import sys

evm_loader = PublicKey(sys.argv[1])
mint = PublicKey(sys.argv[2])

signer = OperatorAccount().get_acc()
authority_account = PublicKey.find_program_address([b"Deposit"], evm_loader)[0]
pool = get_associated_token_address(authority_account, mint)
print("Pool: ", pool)

trx = Transaction()
trx.add(create_associated_token_account(signer.public_key(), authority_account, mint))
result = send_transaction(client, trx, signer)
print(result)