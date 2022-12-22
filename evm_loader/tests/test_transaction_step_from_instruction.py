import random
import string

import eth_abi
import pytest
from eth_keys import keys as eth_keys
from eth_utils import abi
from solana.keypair import Keypair
from solana.publickey import PublicKey

from .solana_utils import get_neon_balance, solana_client, execute_transaction_steps_from_instruction, neon_cli
from .utils.constants import TAG_FINALIZED_STATE
from .utils.contract import make_deployment_transaction, deploy_contract
from .utils.ethereum import make_eth_transaction, create_contract_address
from .utils.layouts import FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder
from .utils.transaction_checks import check_transaction_logs_have_text, check_holder_account_tag


class TestTransactionStepFromInstruction:

    def test_simple_transfer_transaction(self, operator_keypair, treasury_pool, evm_loader,
                                         sender_with_tokens, second_user, holder_acc):
        amount = 10
        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_before = get_neon_balance(solana_client, second_user.solana_account_address)

        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)

        resp = execute_transaction_steps_from_instruction(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                          signed_tx, [second_user.solana_account_address,
                                                                      sender_with_tokens.solana_account_address], 0)

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_after = get_neon_balance(solana_client, second_user.solana_account_address)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")
        assert sender_balance_before - amount == sender_balance_after
        assert recipient_balance_before + amount == recipient_balance_after

    @pytest.mark.parametrize("chain_id", [None, 111])
    def test_deploy_contract(self, operator_keypair, holder_acc, treasury_pool, evm_loader, sender_with_tokens, chain_id):
        contract_filename = "small.binary"

        signed_tx = make_deployment_transaction(sender_with_tokens, contract_filename, chain_id=chain_id)
        contract = create_contract_address(sender_with_tokens, evm_loader)

        contract_path = pytest.CONTRACTS_PATH / contract_filename
        with open(contract_path, 'rb') as f:
            contract_code = f.read()

        steps_count = neon_cli().get_steps_count(evm_loader, sender_with_tokens, "deploy", contract_code.hex())
        resp = execute_transaction_steps_from_instruction(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                          signed_tx, [contract.solana_address,
                                                                      sender_with_tokens.solana_account_address],
                                                          steps_count)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value, "exit_status=0x12")

    def test_call_contract_function_without_neon_transfer(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                          evm_loader, holder_acc):
        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)
        text = ''.join(random.choice(string.ascii_letters) for i in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address)
        steps_count = neon_cli().get_steps_count(evm_loader, sender_with_tokens, contract, data.hex())
        resp = execute_transaction_steps_from_instruction(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                          signed_tx, [contract.solana_address,
                                                                      sender_with_tokens.solana_account_address],
                                                          steps_count)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        assert text in neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()")

    def test_call_contract_function_with_neon_transfer(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                       evm_loader, holder_acc):
        transfer_amount = random.randint(1, 1000)

        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)

        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_before = get_neon_balance(solana_client, contract.solana_address)

        text = ''.join(random.choice(string.ascii_letters) for i in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, value=transfer_amount)
        steps_count = neon_cli().get_steps_count(evm_loader, sender_with_tokens, contract, data.hex())
        resp = execute_transaction_steps_from_instruction(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                          signed_tx, [contract.solana_address,
                                                                      sender_with_tokens.solana_account_address],
                                                          steps_count)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_after = get_neon_balance(solana_client, contract.solana_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

        assert text in neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()")

    def test_transfer_transaction_with_non_existing_recipient(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                              evm_loader, holder_acc):
        # recipient account should be created
        recipient = Keypair.generate()
        recipient_ether = eth_keys.PrivateKey(recipient.secret_key[:32]).public_key.to_canonical_address()
        recipient_solana_address, _ = evm_loader.ether2program(recipient_ether)
        amount = 10
        signed_tx = make_eth_transaction(recipient_ether, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)

        resp = execute_transaction_steps_from_instruction(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                          signed_tx, [PublicKey(recipient_solana_address),
                                                                      sender_with_tokens.solana_account_address], 0)

        recipient_balance_after = get_neon_balance(solana_client, PublicKey(recipient_solana_address))
        check_transaction_logs_have_text(resp.value, "ExitSucceed")

        assert recipient_balance_after == amount
