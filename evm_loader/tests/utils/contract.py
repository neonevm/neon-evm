import random
import typing as tp
import pathlib

import pytest
from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.rpc.types import TxOpts
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction

from ..solana_utils import EVM_LOADER, solana_client, \
    send_transaction, create_account_with_seed, get_transaction_count, EvmLoader, \
    wait_confirm_transaction
from .storage import create_holder, create_holder_account
from .instructions import TransactionWithComputeBudget, make_WriteHolder, make_CreateAccountV02, make_ExecuteTrxFromAccountDataIterativeOrContinue
from ..conftest import Caller, TreasuryPool
from .ethereum import create_contract_address, Contract

from web3.auto import w3

def write_transaction_to_holder_account(user: Caller, contract_path: tp.Union[pathlib.Path, str], holder_account: PublicKey, operator: Keypair) -> int:
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    if not contract_path.name.startswith("/") or not contract_path.name.startswith("."):
        contract_path = pytest.CONTRACTS_PATH / contract_path
    with open(contract_path, 'rb') as f:
        contract_code = f.read()

    tx = {
        'to': None,
        'value': 0,
        'gas': 999999999,
        'gasPrice': 0,
        'nonce': get_transaction_count(solana_client, user.solana_account_address),
        'data': contract_code,
        'chainId': 111
    }

    signed_tx = w3.eth.account.sign_transaction(tx, user.solana_account.secret_key[:32])

    # Write transaction to transaction holder account
    offset = 0
    receipts = []
    rest = signed_tx.rawTransaction
    while len(rest):
        (part, rest) = (rest[:920], rest[920:])
        trx = Transaction()
        trx.add(make_WriteHolder(operator.public_key, holder_account, signed_tx.hash, offset, part))
        receipts.append(
            solana_client.send_transaction(trx, operator, opts=TxOpts(
                skip_confirmation=True, preflight_commitment=Confirmed))["result"])
        offset += len(part)
    for rcpt in receipts:
        wait_confirm_transaction(solana_client, rcpt)
    return offset


def create_contract_accounts(seed: str, code_size: int, contract: Contract, operator: Keypair):
    print("Creating contract accounts")
    trx = Transaction()
    trx.add(
        create_account_with_seed(operator.public_key, operator.public_key, seed, 10 ** 9,
                                 code_size, PublicKey(EVM_LOADER)))

    trx.add(make_CreateAccountV02(operator, contract.solana_address, contract.eth_address, contract.nonce, contract.code_solana_address))
    receipt = send_transaction(solana_client, trx, operator)["result"]
    return receipt


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
        [contract.solana_address, contract.code_solana_address, user.solana_account_address]
    ))
    receipt = send_transaction(solana_client, trx, operator)["result"]
    return receipt


def deploy_contract(operator: Keypair, user: Caller, contract_path: tp.Union[pathlib.Path, str], evm_loader: EvmLoader, treasury_pool: TreasuryPool, step_count: int = 1000):
    print("Deploying contract")
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    # storage_account = create_storage_account(operator)
    contract = create_contract_address(user, evm_loader)
    holder_acc = create_holder(operator)
    size = write_transaction_to_holder_account(user, contract_path, holder_acc, operator)
    create_contract_accounts(contract.seed, size + 1 + 32 + 4 + 2048, contract, operator)

    contract_deployed = False
    while not contract_deployed:
        receipt = deploy_contract_step(step_count, treasury_pool, holder_acc, operator, evm_loader, contract, user)
        if receipt["meta"]["err"]:
            raise AssertionError(f"Can't deploy contract: {receipt['meta']['err']}")
        for log in receipt["meta"]["logMessages"]:
            if "exit_status" in log:
                contract_deployed = True
                break
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
    return contract
