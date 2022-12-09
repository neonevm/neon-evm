import base64
import json
import random
import string

import pytest
import solana
import eth_abi
from eth_utils import abi, to_text

from .solana_utils import make_new_user, execute_trx_from_instruction, deposit_neon, solana_client, get_neon_balance, \
    neon_cli
from .utils.contract import make_deployment_transaction, deploy_contract
from .utils.ethereum import make_eth_transaction, create_contract_address


def check_transaction_logs_have_text(trx_hash, text):
    receipt = solana_client.get_transaction(trx_hash)
    logs = ""
    for log in receipt.value.transaction.meta.log_messages:
        print(log)
        if "Program data:" in log:
            logs += "Program data: " + str(base64.b64decode(log.replace("Program data: ", "")))
        else:
            logs += log
        logs += " "
    print(logs)
    assert text in logs, f"Transaction logs don't contain '{text}'. Logs: {logs}"


def test_simple_transfer_transaction(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)
    amount = 1000
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, amount)
    sender_balance_before = get_neon_balance(solana_client, user_account.solana_account_address)
    recipient_balance_before = get_neon_balance(solana_client, recipient.solana_account_address)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, amount)
    resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                        signed_tx,
                                        [user_account.solana_account_address,
                                         recipient.solana_account_address],
                                        operator_keypair)
    sender_balance_after = get_neon_balance(solana_client, user_account.solana_account_address)
    recipient_balance_after = get_neon_balance(solana_client, recipient.solana_account_address)
    assert sender_balance_before - amount == sender_balance_after
    assert recipient_balance_before + amount == recipient_balance_after
    check_transaction_logs_have_text(resp.value, "ExitSucceed")


def test_deploy_contract(operator_keypair, evm_loader, treasury_pool, user_account):
    contract_filename = "small.binary"

    signed_tx = make_deployment_transaction(user_account, contract_filename)
    contract = create_contract_address(user_account, evm_loader)

    with pytest.raises(solana.rpc.core.RPCException, match="Deploy transactions are not allowed"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account,
                                     treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      contract.solana_address],
                                     operator_keypair)


@pytest.mark.parametrize("chain_id", [None, 111])
def test_call_contract_function_without_neon_transfer(operator_keypair, treasury_pool, user_account, evm_loader,
                                                      chain_id):
    contract = deploy_contract(operator_keypair, user_account, "string_setter.binary", evm_loader, treasury_pool)
    text = ''.join(random.choice(string.ascii_letters) for i in range(10))
    func_name = abi.function_signature_to_4byte_selector('set(string)')
    data = func_name + eth_abi.encode(['string'], [text])
    signed_tx = make_eth_transaction(contract.eth_address, data, user_account.solana_account,
                                     user_account.solana_account_address, chain_id=None)
    resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                        signed_tx,
                                        [user_account.solana_account_address,
                                         contract.solana_address],
                                        operator_keypair)

    check_transaction_logs_have_text(resp.value, "ExitSucceed")

    data = abi.function_signature_to_4byte_selector('get()')
    result = json.loads(
        neon_cli().emulate(evm_loader.loader_id, user_account.eth_address.hex(), contract.eth_address.hex(), data.hex())
    )
    assert text in to_text(result["result"])


def test_call_contract_function_with_neon_transfer(operator_keypair, treasury_pool, user_account, evm_loader):
    transfer_amount = random.randint(1, 100000)
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, transfer_amount)

    contract = deploy_contract(operator_keypair, user_account, "string_setter.binary", evm_loader, treasury_pool)

    sender_balance_before = get_neon_balance(solana_client, user_account.solana_account_address)
    contract_balance_before = get_neon_balance(solana_client, contract.solana_address)

    text = ''.join(random.choice(string.ascii_letters) for i in range(10))
    func_name = abi.function_signature_to_4byte_selector('set(string)')
    data = func_name + eth_abi.encode(['string'], [text])
    signed_tx = make_eth_transaction(contract.eth_address, data, user_account.solana_account,
                                     user_account.solana_account_address, transfer_amount)
    resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                        signed_tx,
                                        [user_account.solana_account_address,
                                         contract.solana_address],
                                        operator_keypair)

    check_transaction_logs_have_text(resp.value, "ExitSucceed")

    data = abi.function_signature_to_4byte_selector('get()')
    result = json.loads(
        neon_cli().emulate(evm_loader.loader_id, user_account.eth_address.hex(), contract.eth_address.hex(), data.hex())
    )
    print(to_text(result["result"]))
    assert text in to_text(result["result"])
    sender_balance_after = get_neon_balance(solana_client, user_account.solana_account_address)
    contract_balance_after = get_neon_balance(solana_client, contract.solana_address)
    assert sender_balance_before - transfer_amount == sender_balance_after
    assert contract_balance_before + transfer_amount == contract_balance_after


def test_incorrect_chain_id(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)
    amount = 1
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, amount)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, amount, chain_id=1)
    with pytest.raises(solana.rpc.core.RPCException, match="Invalid chain_id"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      recipient.solana_account_address],
                                     operator_keypair)


def test_incorrect_nonce(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)
    amount = 1
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, amount)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, amount)

    execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                 signed_tx,
                                 [user_account.solana_account_address,
                                  recipient.solana_account_address],
                                 operator_keypair)
    with pytest.raises(solana.rpc.core.RPCException, match="Invalid Ethereum transaction nonce"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      recipient.solana_account_address],
                                     operator_keypair)


def test_insufficient_funds(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, 10)

    with pytest.raises(solana.rpc.core.RPCException, match="insufficient funds for instruction"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      recipient.solana_account_address],
                                     operator_keypair)


def test_gas_limit_reached(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)
    amount = 10
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, amount)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, amount, gas=1)

    with pytest.raises(solana.rpc.core.RPCException, match="Out of gas used"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      recipient.solana_account_address],
                                     operator_keypair)

def test_incorrect_sender(operator_keypair, treasury_pool, user_account, evm_loader):
    recipient = make_new_user(evm_loader)
    amount = 10
    deposit_neon(evm_loader, operator_keypair, user_account.eth_address, amount)

    signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                     user_account.solana_account_address, amount, gas=1)

    with pytest.raises(solana.rpc.core.RPCException, match="Out of gas used"):
        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [user_account.solana_account_address,
                                      recipient.solana_account_address],
                                     operator_keypair)
