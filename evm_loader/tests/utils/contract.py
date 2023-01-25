import typing as tp
import pathlib

import eth_abi
import pytest
from eth_account.datastructures import SignedTransaction
from eth_utils import abi
from solana.keypair import Keypair

from .types import Caller, TreasuryPool
from ..solana_utils import solana_client, \
    get_transaction_count, EvmLoader, write_transaction_to_holder_account, \
    send_transaction_step_from_account, neon_cli
from .storage import create_holder
from .ethereum import create_contract_address, make_eth_transaction

from web3.auto import w3


def make_deployment_transaction(
        user: Caller,
        contract_path: tp.Union[pathlib.Path, str],
        encoded_args=None,
        gas: int = 999999999, chain_id=111
) -> SignedTransaction:
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    if not contract_path.name.startswith("/") or not contract_path.name.startswith("."):
        contract_path = pytest.CONTRACTS_PATH / contract_path
    with open(contract_path, 'rb') as f:
        data = f.read()

    if encoded_args is not None:
        data += encoded_args

    tx = {
        'to': None,
        'value': 0,
        'gas': gas,
        'gasPrice': 0,
        'nonce': get_transaction_count(solana_client, user.solana_account_address),
        'data': data
    }
    if chain_id is not None:
        tx['chainId'] = chain_id
    print(tx)
    return w3.eth.account.sign_transaction(tx, user.solana_account.secret_key[:32])


def make_contract_call_trx(user, contract, function_signature, params=None, value=0, chain_id=111):
    data = abi.function_signature_to_4byte_selector(function_signature)

    if params is not None:
        for param in params:
            if isinstance(param, int):
                data += eth_abi.encode(['uint256'], [param])
            elif isinstance(param, str):
                data += eth_abi.encode(['string'], [param])

    signed_tx = make_eth_transaction(contract.eth_address, data, user.solana_account, user.solana_account_address,
                                     value=value, chain_id=chain_id)
    return signed_tx


def deploy_contract(
        operator: Keypair,
        user: Caller,
        contract_path: tp.Union[pathlib.Path, str],
        evm_loader: EvmLoader,
        treasury_pool: TreasuryPool,
        step_count: int = 1000,
        encoded_args=None
):
    print("Deploying contract")
    if isinstance(contract_path, str):
        contract_path = pathlib.Path(contract_path)
    contract = create_contract_address(user, evm_loader)
    holder_acc = create_holder(operator)
    signed_tx = make_deployment_transaction(user, contract_path, encoded_args=encoded_args)
    write_transaction_to_holder_account(signed_tx, holder_acc, operator)

    contract_deployed = False
    while not contract_deployed:
        receipt = send_transaction_step_from_account(operator, evm_loader, treasury_pool, holder_acc,
                                                     [contract.solana_address, user.solana_account_address],
                                                     step_count, operator)
        if receipt.value.transaction.meta.err:
            raise AssertionError(f"Can't deploy contract: {receipt.value.transaction.meta.err}")
        for log in receipt.value.transaction.meta.log_messages:
            if "exit_status" in log:
                contract_deployed = True
                break
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
    return contract
