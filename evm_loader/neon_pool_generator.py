import sys

from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.rpc.types import TxOpts
from solana.rpc.commitment import Confirmed
from tests.solana_utils import OperatorAccount, create_treasury_pool_address, get_associated_token_address, solana_client, \
    TransactionWithComputeBudget, create_associated_token_account, send_transaction, \
    create_treasury_pool_address, wait_confirm_transaction
from solana.rpc.commitment import Processed
from solana.system_program import SYS_PROGRAM_ID, transfer, TransferParams

evm_loader = PublicKey(sys.argv[1])
mint = PublicKey(sys.argv[2])
treasury_count = int(sys.argv[3])
create_mode = (len(sys.argv) > 4 and sys.argv[4]=='create')
all_exists = True

signer = OperatorAccount().get_acc()

print("Check treasury pool balances")
min_lamports = solana_client.get_minimum_balance_for_rent_exemption(usize=0)['result']
trx_results = []
for index in range(treasury_count):
    treasury_account = create_treasury_pool_address(index, evm_loader=evm_loader)
    treasury_account_info = solana_client.get_account_info(treasury_account)['result']['value']
    lamports = treasury_account_info['lamports'] if treasury_account_info is not None else 0
    if lamports >= min_lamports:
        print(f'{index} {treasury_account} Ok {lamports}')
    elif create_mode:
        trx = TransactionWithComputeBudget()
        trx.add(transfer(TransferParams(
            from_pubkey=signer.public_key(),
            to_pubkey=treasury_account,
            lamports=min_lamports-lamports
        )))
        result = solana_client.send_transaction(trx, Keypair.from_secret_key(signer.secret_key()),
            opts=TxOpts(skip_confirmation=True, skip_preflight=False, preflight_commitment=Confirmed))
        trx_results.append(result['result'])
        print(f'{index} {treasury_account} Funded {result}')
    else:
        print(f'{index} {treasury_account} Missed {min_lamports-lamports}')
        all_exists = False
for trx in trx_results:
    wait_confirm_transaction(solana_client, trx, confirmations=1)

print("Check NeonEVM bank balance for Neon")
authority_account = PublicKey.find_program_address([b"Deposit"], evm_loader)[0]
pool = get_associated_token_address(authority_account, mint)
print("Pool: ", pool)
pool_account = solana_client.get_account_info(pool, commitment=Processed)["result"]["value"]
if pool_account is not None:
    print("Pool account already exists")
elif create_mode:
    trx = TransactionWithComputeBudget()
    trx.add(create_associated_token_account(signer.public_key(), authority_account, mint))
    result = send_transaction(solana_client, trx, Keypair.from_secret_key(signer.secret_key()))
    print(result)
else:
    print("Pool account doesn't exist")
    all_exists = False

exit(0 if all_exists else 1)