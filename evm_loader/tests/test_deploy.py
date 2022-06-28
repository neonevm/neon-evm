import json
from eth_utils import abi, to_text

from .solana_utils import get_solana_balance, neon_cli
from .utils.contract import deploy_contract


def test_deploy_contract(user_account, evm_loader, operator_keypair, treasury_pool):
    contract = deploy_contract(operator_keypair, user_account, "hello_world.binary", evm_loader, treasury_pool)
    assert contract.eth_address
    assert get_solana_balance(contract.solana_address) > 0
    data = abi.function_signature_to_4byte_selector('call_hello_world()')
    result = json.loads(neon_cli().emulate(evm_loader.loader_id, f"{user_account.eth_address.hex()} {contract.eth_address.hex()} {data.hex()}"))
    assert result["exit_status"] == "succeed"
    assert "Hello World" in to_text(result["result"])
