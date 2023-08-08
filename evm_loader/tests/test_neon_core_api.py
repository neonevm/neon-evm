import pytest
from eth_utils import abi, to_text

from .utils.contract import deploy_contract
from .solana_utils import solana_client


def test_get_storage_at(neon_api_client, operator_keypair, user_account, evm_loader, treasury_pool):
    contract = deploy_contract(operator_keypair, user_account, "hello_world.binary", evm_loader, treasury_pool)
    storage = neon_api_client.get_storage_at(contract.eth_address.hex())["value"]
    zero_array = [0 for _ in range(31)]
    assert storage == zero_array + [5]

    storage = neon_api_client.get_storage_at(contract.eth_address.hex(), index='0x2')["value"]
    assert storage == zero_array + [0]


def test_get_ether_account_data(neon_api_client, user_account):
    result = neon_api_client.get_ether_account_data(user_account.eth_address.hex())['value']
    assert f"0x{user_account.eth_address.hex()}" == result["address"]
    assert str(user_account.solana_account_address) == result["solana_address"]
    assert solana_client.get_account_info(user_account.solana_account.public_key).value is not None


def test_emulate_transfer(neon_api_client, user_account, session_user):
    result = neon_api_client.emulate(user_account.eth_address.hex(),
                                     session_user.eth_address.hex())["value"]
    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] == 1, f"Steps executed amount is not 1. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"


def test_emulate_contract_deploy(neon_api_client, user_account):
    contract_path = pytest.CONTRACTS_PATH / "hello_world.binary"

    with open(contract_path, 'rb') as f:
        contract_code = f.read()
    result = neon_api_client.emulate(user_account.eth_address.hex(),
                                     contract=None, data=contract_code)["value"]
    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] > 100, f"Steps executed amount is wrong. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"


def test_emulate_call_contract_function(neon_api_client, operator_keypair, treasury_pool, evm_loader, user_account):
    contract = deploy_contract(operator_keypair, user_account, "hello_world.binary", evm_loader, treasury_pool)
    assert contract.eth_address
    data = abi.function_signature_to_4byte_selector('call_hello_world()')

    result = neon_api_client.emulate(user_account.eth_address.hex(),
                                     contract=contract.eth_address.hex(), data=data)["value"]

    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] > 0, f"Steps executed amount is 0. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"
    assert "Hello World" in to_text(result["result"])


def test_emulate_with_small_amount_of_steps(neon_api_client, evm_loader, user_account):
    contract_path = pytest.CONTRACTS_PATH / "hello_world.binary"
    with open(contract_path, 'rb') as f:
        contract_code = f.read()
    result = neon_api_client.emulate(user_account.eth_address.hex(),
                                     contract=None, data=contract_code, max_steps_to_execute=10)
    assert result['error'] == 'Too many steps'
