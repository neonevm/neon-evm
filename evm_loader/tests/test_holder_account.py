from hashlib import sha256
from random import randrange

import pytest
import solana
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction

from . import solana_utils
from .solana_utils import solana_client, write_transaction_to_holder_account, \
    send_transaction_step_from_account, get_solana_balance, execute_transaction_steps_from_account
from .utils.constants import EVM_LOADER, TAG_STATE, TAG_FINALIZED_STATE
from .utils.contract import make_deployment_transaction, make_contract_call_trx
from .utils.ethereum import make_eth_transaction
from .utils.layouts import STORAGE_ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder, delete_holder


def test_create_holder_account(operator_keypair):
    holder_acc = create_holder(operator_keypair)
    info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
    assert info.value is not None, "Holder account is not created"
    assert info.value.lamports == 1000000000, "Account balance is not correct"


def test_create_the_same_holder_account_by_another_user(operator_keypair, session_user):
    seed = str(randrange(1000000))
    storage = PublicKey(
        sha256(bytes(operator_keypair.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())
    create_holder(operator_keypair, seed=seed, storage=storage)

    trx = Transaction()
    trx.add(
        solana_utils.create_account_with_seed(session_user.solana_account.public_key,
                                              session_user.solana_account.public_key, seed, 10 ** 9, 128 * 1024),
        solana_utils.create_holder_account(storage, session_user.solana_account.public_key)
    )

    with pytest.raises(solana.rpc.core.RPCException, match='already initialized'):
        solana_utils.send_transaction(solana_client, trx, session_user.solana_account)


def test_write_tx_to_holder(operator_keypair, session_user, second_session_user, evm_loader):
    holder_acc = create_holder(operator_keypair)
    signed_tx = make_eth_transaction(second_session_user.eth_address, None, session_user.solana_account,
                                     session_user.solana_account_address, 10)
    write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
    info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
    assert signed_tx.rawTransaction == info.value.data[65:65 + len(signed_tx.rawTransaction)], \
        "Account data is not correct"


def test_write_tx_to_holder_in_parts(operator_keypair, session_user):
    holder_acc = create_holder(operator_keypair)
    contract_filename = "ERC20ForSplFactory.binary"

    signed_tx = make_deployment_transaction(session_user, contract_filename)
    write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
    info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
    assert signed_tx.rawTransaction == info.value.data[65:65 + len(signed_tx.rawTransaction)], \
        "Account data is not correct"


def test_write_tx_to_holder_by_no_owner(operator_keypair, session_user, second_session_user, evm_loader):
    holder_acc = create_holder(operator_keypair)

    signed_tx = make_eth_transaction(second_session_user.eth_address, None, session_user.solana_account,
                                     session_user.solana_account_address, 10)
    with pytest.raises(solana.rpc.core.RPCException, match="invalid account data for instruction"):
        write_transaction_to_holder_account(signed_tx, holder_acc, session_user.solana_account)


def test_delete_holder(operator_keypair):
    holder_acc = create_holder(operator_keypair)
    delete_holder(holder_acc, operator_keypair, operator_keypair)
    info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
    assert info.value is None, "Holder account isn't deleted"


def test_success_refund_after_holder_deliting(operator_keypair):
    holder_acc = create_holder(operator_keypair)

    pre_storage = get_solana_balance(holder_acc)
    pre_acc = get_solana_balance(operator_keypair.public_key)

    delete_holder(holder_acc, operator_keypair, operator_keypair)

    post_acc = get_solana_balance(operator_keypair.public_key)

    assert pre_storage + pre_acc, post_acc + 5000


def test_delete_holder_by_no_owner(operator_keypair, user_account):
    holder_acc = create_holder(operator_keypair)
    with pytest.raises(solana.rpc.core.RPCException, match="invalid account data for instruction"):
        delete_holder(holder_acc, user_account.solana_account, user_account.solana_account)


def test_write_to_not_finalized_holder(rw_lock_contract, user_account, evm_loader, operator_keypair, treasury_pool,
                                       new_holder_acc):
    signed_tx = make_contract_call_trx(user_account, rw_lock_contract, "unchange_storage(uint8,uint8)", [1, 1])
    write_transaction_to_holder_account(signed_tx, new_holder_acc, operator_keypair)

    send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                       [user_account.solana_account_address,
                                        rw_lock_contract.solana_address], 1, operator_keypair)
    account_data = solana_client.get_account_info(new_holder_acc).value.data
    parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
    assert parsed_data.tag == TAG_STATE

    signed_tx2 = make_contract_call_trx(user_account, rw_lock_contract, "unchange_storage(uint8,uint8)", [1, 1])

    with pytest.raises(solana.rpc.core.RPCException, match=r"Account .* - expected Holder or Finalized"):
        write_transaction_to_holder_account(signed_tx2, new_holder_acc, operator_keypair)


def test_write_to_finalized_holder(rw_lock_contract, session_user, evm_loader, operator_keypair, treasury_pool,
                                   new_holder_acc):
    signed_tx = make_contract_call_trx(session_user, rw_lock_contract, "unchange_storage(uint8,uint8)", [1, 1])
    write_transaction_to_holder_account(signed_tx, new_holder_acc, operator_keypair)

    execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                           [session_user.solana_account_address,
                                            rw_lock_contract.solana_address])
    signed_tx2 = make_contract_call_trx(session_user, rw_lock_contract, "unchange_storage(uint8,uint8)", [1, 1])

    write_transaction_to_holder_account(signed_tx2, new_holder_acc, operator_keypair)
    info = solana_client.get_account_info(new_holder_acc, commitment=Confirmed)
    assert signed_tx2.rawTransaction == info.value.data[65:65 + len(signed_tx2.rawTransaction)], \
        "Account data is not correct"
