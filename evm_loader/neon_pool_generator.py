import sys

from solana.publickey import PublicKey
from solana.keypair import Keypair
from tests.solana_utils import OperatorAccount, get_associated_token_address, solana_client, TransactionWithComputeBudget, create_associated_token_account, send_transaction
from solana.rpc.commitment import Processed


evm_loader = PublicKey(sys.argv[1])
mint = PublicKey(sys.argv[2])
mode = sys.argv[3] or 'create'

signer = OperatorAccount().get_acc()
authority_account = PublicKey.find_program_address([b"Deposit"], evm_loader)[0]
pool = get_associated_token_address(authority_account, mint)
print("Pool: ", pool)

pool_account = solana_client.get_account_info(pool, commitment=Processed)["result"]["value"]
if pool_account is not None:
    print("Pool account already exists")
    exit(0)

if mode != 'create':
    exit(1)

trx = TransactionWithComputeBudget()
trx.add(create_associated_token_account(signer.public_key(), authority_account, mint))
result = send_transaction(solana_client, trx, Keypair.from_secret_key(signer.secret_key()))
print(result)