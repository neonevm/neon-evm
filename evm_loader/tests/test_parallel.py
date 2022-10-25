import json
from typing import Any

from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Finalized
from solana.rpc.core import RPCException
from solana.transaction import Transaction

from .solana_utils import EvmLoader, send_transaction, solana_client, get_account_data, make_new_user, deposit_neon
from .utils.contract import write_transaction_to_holder_account, deploy_contract_step
from .utils.ethereum import create_contract_address, make_eth_transfer_transaction
from .utils.instructions import make_Cancel, TransactionWithComputeBudget, make_ExecuteTrxFromInstruction
from .utils.layouts import ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder
from .utils.types import Caller, TreasuryPool

EVM_STEPS_COUNT = 0xFFFFFFFF_FFFFFFFF
BILLION = 1_000_000_000

def test_create_same_accounts(
    user_account: Caller,
    evm_loader: EvmLoader,
    operator_keypair: Keypair,
    treasury_pool: TreasuryPool,
):
    second_account = make_new_user(evm_loader)
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, BILLION)
    deposit_neon(evm_loader, operator_keypair, second_account.eth_address, BILLION)

    contract = create_contract_address(user_account, evm_loader)
    holder_acc = create_holder(operator_keypair)
    _size, deployment_tx_hash = \
        write_transaction_to_holder_account(user_account, "ERC20ForSplFactory.binary", holder_acc, operator_keypair)

    # First 2 iterations
    for i in range(2):
        deployment_receipt = deploy_contract_step(
            EVM_STEPS_COUNT,
            treasury_pool,
            holder_acc,
            operator_keypair,
            evm_loader,
            contract,
            user_account,
        )

        assert not check_iteration_deployed(deployment_receipt)

    # Transferring to the same account in order to break deployment
    transfer(
        second_account,
        contract.eth_address,
        BILLION,
        contract.solana_address,
        evm_loader,
        operator_keypair,
        treasury_pool,
    )

    # Trying to finish deployment (expected to fail)
    try:
        deploy_contract_step(
            EVM_STEPS_COUNT,
            treasury_pool,
            holder_acc,
            operator_keypair,
            evm_loader,
            contract,
            user_account,
        )

        assert False, 'Deployment expected to fail'
    except RPCException as e:
        error = json.loads(str(e).replace('\'', '\"').replace('None', 'null'))
        assert error['code'] == -32002
        assert 'instruction requires an uninitialized account' in error['message']
        assert 'Blocked nonexistent account %s was created/initialized outside current transaction' \
               % contract.solana_address

    # Cancel deployment transaction:
    trx = Transaction()
    trx.add(
        make_Cancel(
            holder_acc,
            operator_keypair,
            deployment_tx_hash,
            [contract.solana_address, user_account.solana_account_address],
        )
    )
    cancel_receipt = send_transaction(solana_client, trx, operator_keypair)
    print("Cancel receipt:", cancel_receipt)
    assert cancel_receipt["result"]["meta"]["err"] is None

    expected_length = ACCOUNT_INFO_LAYOUT.sizeof()
    data = get_account_data(solana_client, contract.solana_address, expected_length)
    assert len(data) == expected_length

    account = ACCOUNT_INFO_LAYOUT.parse(data)
    assert account.code_size == 0
    balance = int.from_bytes(account.balance, byteorder="little")
    assert balance == BILLION


def check_iteration_deployed(receipt: Any) -> bool:
    if receipt["meta"]["err"]:
        raise AssertionError(f"Can't deploy contract: {receipt['meta']['err']}")
    for log in receipt["meta"]["logMessages"]:
        if "exit_status" in log:
            return True
        if "ExitError" in log:
            raise AssertionError(f"EVM Return error in logs: {receipt}")
    return False


def transfer(
    src_account: Caller,
    dst_addr: bytes,
    value: int,
    dst_solana_addr: PublicKey,
    evm_loader: EvmLoader,
    operator_keypair: Keypair,
    treasury_pool: TreasuryPool,
):
    message = make_eth_transfer_transaction(src_account, dst_addr, value).rawTransaction

    trx = TransactionWithComputeBudget()
    trx.add(
        make_ExecuteTrxFromInstruction(
            operator_keypair,
            evm_loader,
            treasury_pool.account,
            treasury_pool.buffer,
            message,
            [dst_solana_addr, src_account.solana_account_address],
        )
    )
    receipt = send_transaction(solana_client, trx, operator_keypair, Finalized)
    print("Transfer receipt:", receipt)
    assert "success" in receipt["result"]["meta"]["logMessages"][-1]

    return receipt
