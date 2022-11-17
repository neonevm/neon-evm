import json
import os

import pytest
from solana.rpc.api import Client

from .solana_utils import neon_cli, get_solana_balance, send_transaction
from .utils.constants import SOLANA_URL
from .utils.contract import deploy_contract
from .utils.ethereum import make_eth_transaction
from eth_utils import abi, to_text

from .utils.instructions import TransactionWithComputeBudget, make_PartialCallOrContinueFromRawEthereumTX
from .utils.storage import create_holder


def gen_hash_of_block(size: int) -> str:
    """Generates a block hash of the given size"""
    try:
        block_hash = hex(int.from_bytes(os.urandom(size), "big"))
        if bytes.fromhex(block_hash[2:]):
            return block_hash
    except ValueError:
        return gen_hash_of_block(size)


def test_emulate_transfer(user_account, evm_loader, second_user):
    result = json.loads(
        neon_cli().emulate(
            evm_loader.loader_id,
            user_account.eth_address.hex(),
            second_user.eth_address.hex(),
            data=None
        )
    )
    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] == 0, f"Steps executed amount is not 0. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"


def test_emulate_contract_deploy(user_account, evm_loader):
    contract_path = pytest.CONTRACTS_PATH / "hello_world.binary"

    with open(contract_path, 'rb') as f:
        contract_code = f.read()

    result = json.loads(
        neon_cli().emulate(
            evm_loader.loader_id,
            user_account.eth_address.hex(),
            'deploy',
            contract_code.hex()
        )
    )
    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] > 0, f"Steps executed amount is not 0. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"


def test_emulate_call_contract_function(user_account, evm_loader, operator_keypair, treasury_pool):
    contract = deploy_contract(operator_keypair, user_account, "hello_world.binary", evm_loader, treasury_pool)
    assert contract.eth_address
    assert get_solana_balance(contract.solana_address) > 0
    data = abi.function_signature_to_4byte_selector('call_hello_world()')
    result = json.loads(
        neon_cli().emulate(
            evm_loader.loader_id,
            user_account.eth_address.hex(),
            contract.eth_address.hex(),
            data.hex()
        )
    )
    assert result['exit_status'] == 'succeed', f"The 'exit_status' field is not succeed. Result: {result}"
    assert result['steps_executed'] > 0, f"Steps executed amount is not 0. Result: {result}"
    assert result['used_gas'] > 0, f"Used gas is less than 0. Result: {result}"
    assert "Hello World" in to_text(result["result"])


def test_neon_elf_params(evm_loader):
    result = neon_cli().call(f"--evm_loader={evm_loader.loader_id} neon-elf-params").strip()
    some_fields = ['NEON_CHAIN_ID', 'NEON_TOKEN_MINT', 'NEON_REVISION']
    param_dict = {}
    for param in result.split('\n'):
        param_dict[param.split('=')[0]] = param.split('=')[1]
    for field in some_fields:
        assert field in param_dict, f"The field {field} is not in result {result}"
        assert param_dict[field] != "", f"The value for fiels {field} is empty"


def test_help():
    result = neon_cli().call("--help").strip()
    assert len(result) > 1000


def test_collect_treasury(evm_loader):
    result = neon_cli().call(f"collect-treasury --evm_loader {evm_loader.loader_id}")
    assert len(result) > 1000



def test_init_environment(evm_loader):
    result = neon_cli().call(f"init-environment --evm_loader {evm_loader.loader_id}")
    assert len(result) > 1000


def test_get_ether_account_data(evm_loader, user_account):
    result = neon_cli().call(
        f"get-ether-account-data --evm_loader {evm_loader.loader_id} {user_account.eth_address.hex()}")
    assert "Account found" in result


def test_create_ether_account(evm_loader):
    acc = gen_hash_of_block(20)
    result = neon_cli().call(
        f"create-ether-account --evm_loader {evm_loader.loader_id} {acc[2:]}")
    assert "CompiledInstruction" in result


def test_create_program_address(evm_loader):
    seed = gen_hash_of_block(20)
    result = neon_cli().call(
        f"create-program-address --evm_loader {evm_loader.loader_id} {seed[2:]}").strip()
    assert len(result.split(" ")[0]) == 44


def test_deposit(evm_loader, user_account):
    result = neon_cli().call(
        f"deposit --evm_loader {evm_loader.loader_id} 10 {user_account.eth_address.hex()}").strip()
    assert "CompiledInstruction" in result


def test_get_storage_at(evm_loader, operator_keypair, user_account, treasury_pool):
    contract = deploy_contract(operator_keypair, user_account, "hello_world.binary", evm_loader, treasury_pool)
    result = neon_cli().call(
        f"get-storage-at --evm_loader {evm_loader.loader_id} {contract.eth_address.hex()} 0x0").strip()
    assert len(result) > 500


@pytest.mark.xfail(reason="https://neonlabs.atlassian.net/browse/NDEV-957")
def test_cancel_trx(evm_loader,user_account, deployed_contract, operator_keypair, treasury_pool):
    func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
    data = (func_name + bytes.fromhex("%064x" % 0x01) + bytes.fromhex("%064x" % 0x01))
    eth_transaction = make_eth_transaction(
        deployed_contract.eth_address,
        data,
        user_account.solana_account,
        user_account.solana_account_address,
    )
    storage_account = create_holder(operator_keypair)
    instruction = eth_transaction.rawTransaction
    trx = TransactionWithComputeBudget()
    trx.add(
        make_PartialCallOrContinueFromRawEthereumTX(
            instruction,
            operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
            [
                deployed_contract.solana_address,
                user_account.solana_account_address,
            ]
        )
    )
    solana_client = Client(SOLANA_URL)
    receipt = send_transaction(solana_client, trx, operator_keypair)
    assert "success" in receipt["result"]["meta"]["logMessages"][-1]

    result = neon_cli().call(f"cancel-trx --evm_loader={evm_loader.loader_id} {storage_account}")
    assert "CompiledInstruction" in result
