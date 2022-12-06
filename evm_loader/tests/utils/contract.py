import random
import time
import typing as tp
import pathlib

import pytest
from eth_account.datastructures import SignedTransaction
from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.rpc.types import TxOpts
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction
from solders.transaction_status import TransactionConfirmationStatus

from .types import Caller, TreasuryPool
from ..solana_utils import EVM_LOADER, solana_client, \
    send_transaction, create_account_with_seed, get_transaction_count, EvmLoader, \
    wait_confirm_transaction
from .storage import create_holder, create_holder_account
from .instructions import TransactionWithComputeBudget, make_WriteHolder, make_ExecuteTrxFromAccountDataIterativeOrContinue
from .ethereum import create_contract_address, Contract

from web3.auto import w3


def make_deployment_transaction(
    user: Caller,
    contract_path: tp.Union[pathlib.Path, str],
    gas: int = 999999999,
) -> SignedTransaction:
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    if not contract_path.name.startswith("/") or not contract_path.name.startswith("."):
        contract_path = pytest.CONTRACTS_PATH / contract_path
    with open(contract_path, 'rb') as f:
        contract_code = f.read()

    tx = {
        'to': None,
        'value': 0,
        'gas': gas,
        'gasPrice': 0,
        'nonce': get_transaction_count(solana_client, user.solana_account_address),
        'data': contract_code,
        'chainId': 111
    }

    return w3.eth.account.sign_transaction(tx, user.solana_account.secret_key[:32])




def deploy_contract_step(
    step_count: int,
    treasury: TreasuryPool,
    holder_address: PublicKey,
    operator: Keypair,
    evm_loader: EvmLoader,
    contract: Contract,
    user: Caller,
):
    print(f"Deploying contract with {step_count} steps")
    trx = TransactionWithComputeBudget()

    trx.add(make_ExecuteTrxFromAccountDataIterativeOrContinue(
        operator, evm_loader, holder_address, treasury.account, treasury.buffer, step_count,
        [contract.solana_address, user.solana_account_address]
    ))
    print(trx.instructions)
    print("OPERATOR public_key")
    print(operator.public_key)
    receipt = send_transaction(solana_client, trx, operator)
    print("Deployment receipt:", receipt)

    return receipt


def deploy_contract(
    operator: Keypair,
    user: Caller,
    contract_path: tp.Union[pathlib.Path, str],
    evm_loader: EvmLoader,
    treasury_pool: TreasuryPool,
    step_count: int = 1000,
):
    print("Deploying contract")
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    contract = create_contract_address(user, evm_loader)
    holder_acc = create_holder(operator)
    signed_tx = make_deployment_transaction(user, contract_path)
    solana_client.write_transaction_to_holder_account(signed_tx, holder_acc, operator)

    contract_deployed = False
    while not contract_deployed:
        receipt = deploy_contract_step(step_count, treasury_pool, holder_acc, operator, evm_loader, contract, user)
        print(receipt)
        if receipt.value[0].err is not None:
            raise AssertionError(f"Can't deploy contract: {receipt[0].err}")
        if receipt.value[0].confirmation_status in [TransactionConfirmationStatus.Finalized,
                                                    TransactionConfirmationStatus.Confirmed]:
            contract_deployed = True
    return contract

