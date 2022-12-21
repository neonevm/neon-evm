import random
import string
import time

import eth_abi
import pytest
import solana
from eth_keys import keys as eth_keys
from eth_utils import abi
from solana.keypair import Keypair
from solana.publickey import PublicKey

from .solana_utils import get_neon_balance, solana_client, neon_cli, execute_transaction_steps_from_account, \
    write_transaction_to_holder_account, create_treasury_pool_address, send_transaction_step_from_account
from .utils.assert_messages import InstructionAsserts
from .utils.constants import TAG_FINALIZED_STATE, TAG_STATE
from .utils.contract import make_deployment_transaction, deploy_contract, make_contract_call_trx
from .utils.ethereum import make_eth_transaction, create_contract_address
from .utils.layouts import FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, STORAGE_ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder
from .utils.transaction_checks import check_transaction_logs_have_text, check_holder_account_tag
from .utils.types import TreasuryPool


class TestTransactionStepFromAccount:

    def test_simple_transfer_transaction(self, operator_keypair, treasury_pool, evm_loader,
                                         sender_with_tokens, session_user, holder_acc):
        amount = 10
        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_before = get_neon_balance(solana_client, session_user.solana_account_address)

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [session_user.solana_account_address,
                                                       sender_with_tokens.solana_account_address], 0)

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_after = get_neon_balance(solana_client, session_user.solana_account_address)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")
        assert sender_balance_before - amount == sender_balance_after
        assert recipient_balance_before + amount == recipient_balance_after

    @pytest.mark.parametrize("chain_id", [None, 111])
    def test_deploy_contract(self, operator_keypair, holder_acc, treasury_pool, evm_loader, sender_with_tokens,
                             chain_id):
        contract_filename = "hello_world.binary"
        contract = create_contract_address(sender_with_tokens, evm_loader)

        signed_tx = make_deployment_transaction(sender_with_tokens, contract_filename, chain_id=chain_id)

        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        contract_path = pytest.CONTRACTS_PATH / contract_filename
        with open(contract_path, 'rb') as f:
            contract_code = f.read()

        steps_count = neon_cli().get_steps_count(evm_loader, sender_with_tokens, "deploy", contract_code.hex())
        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [contract.solana_address,
                                                       sender_with_tokens.solana_account_address],
                                                      steps_count)
        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12")

    def test_call_contract_function_without_neon_transfer(self, operator_keypair, holder_acc, treasury_pool,
                                                          sender_with_tokens, evm_loader):
        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)
        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address)

        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        steps_count = neon_cli().get_steps_count(evm_loader, sender_with_tokens, contract, data.hex())

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [contract.solana_address,
                                                       sender_with_tokens.solana_account_address], steps_count)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

        assert text in neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()")

    def test_call_contract_function_with_neon_transfer(self, operator_keypair, treasury_pool,
                                                       sender_with_tokens,
                                                       evm_loader):
        transfer_amount = random.randint(1, 1000)

        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)

        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_before = get_neon_balance(solana_client, contract.solana_address)

        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, value=transfer_amount)
        holder_acc = create_holder(operator_keypair)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [contract.solana_address,
                                                       sender_with_tokens.solana_account_address]
                                                      )

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_after = get_neon_balance(solana_client, contract.solana_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

        assert text in neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()")

    def test_transfer_transaction_with_non_existing_recipient(self, operator_keypair, holder_acc, treasury_pool,
                                                              sender_with_tokens, evm_loader):
        # recipient account should be created
        recipient = Keypair.generate()
        recipient_ether = eth_keys.PrivateKey(recipient.secret_key[:32]).public_key.to_canonical_address()
        recipient_solana_address, _ = evm_loader.ether2program(recipient_ether)
        amount = 10
        signed_tx = make_eth_transaction(recipient_ether, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [PublicKey(recipient_solana_address),
                                                       sender_with_tokens.solana_account_address], 0)

        recipient_balance_after = get_neon_balance(solana_client, PublicKey(recipient_solana_address))
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "ExitSucceed")

        assert recipient_balance_after == amount

    def test_incorrect_chain_id(self, operator_keypair, holder_acc, treasury_pool,
                                sender_with_tokens, session_user, evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1, chain_id=1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_CHAIN_ID):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    sender_with_tokens.solana_account_address], 0)

    def test_incorrect_nonce(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader, session_user,
                             holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                               [session_user.solana_account_address,
                                                sender_with_tokens.solana_account_address], 0)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_NONCE):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    sender_with_tokens.solana_account_address], 0)

    def test_run_finalized_transaction(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader,
                                       session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                               [session_user.solana_account_address,
                                                sender_with_tokens.solana_account_address], 0)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.TRX_ALREADY_FINALIZED):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    sender_with_tokens.solana_account_address], 0)

    def test_insufficient_funds(self, operator_keypair, treasury_pool, evm_loader, session_user,
                                holder_acc, user_account):
        signed_tx = make_eth_transaction(session_user.eth_address, None, user_account.solana_account,
                                         user_account.solana_account_address, 10)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INSUFFICIENT_FUNDS):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    user_account.solana_account_address], 0)

    def test_gas_limit_reached(self, operator_keypair, treasury_pool, session_user, evm_loader, sender_with_tokens,
                               holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 10, gas=1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.OUT_OF_GAS):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    sender_with_tokens.solana_account_address], 0)

    def test_sender_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, session_user,
                                                 sender_with_tokens, evm_loader, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address], 0)

    def test_recipient_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, session_user,
                                                    sender_with_tokens, evm_loader, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [sender_with_tokens.solana_account_address], 0)

    def test_incorrect_treasure_pool(self, operator_keypair, sender_with_tokens, evm_loader, session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        index = 2
        treasury = TreasuryPool(index, Keypair().generate().public_key, index.to_bytes(4, 'little'))

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_TREASURE_ACC):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury, holder_acc,
                                                   [sender_with_tokens.solana_account_address,
                                                    session_user.solana_account_address], 0)

    def test_incorrect_treasure_index(self, operator_keypair, sender_with_tokens, evm_loader,
                                      session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        index = 2
        treasury = TreasuryPool(index, create_treasury_pool_address(index), (index + 1).to_bytes(4, 'little'))
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_TREASURE_ACC):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury, holder_acc,
                                                   [sender_with_tokens.solana_account_address,
                                                    session_user.solana_account_address], 0)

    def test_incorrect_operator_account(self, operator_keypair, sender_with_tokens, evm_loader, treasury_pool,
                                        session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        fake_operator = Keypair()
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ACC_NOT_FOUND):
            execute_transaction_steps_from_account(fake_operator, evm_loader, treasury_pool, holder_acc,
                                                   [sender_with_tokens.solana_account_address,
                                                    session_user.solana_account_address], 0)

    def test_operator_is_not_in_white_list(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                           session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.NOT_AUTHORIZED_OPERATOR):
            execute_transaction_steps_from_account(sender_with_tokens.solana_account, evm_loader, treasury_pool,
                                                   holder_acc,
                                                   [sender_with_tokens.solana_account_address,
                                                    session_user.solana_account_address], 0,
                                                   signer=sender_with_tokens.solana_account)

    def test_incorrect_system_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                      session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_sys_program_id = Keypair().public_key
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_SYSTEM_PROGRAM, fake_sys_program_id)):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                               [sender_with_tokens.solana_account_address,
                                                session_user.solana_account_address], 1, operator_keypair,
                                               system_program=fake_sys_program_id)

    def test_incorrect_neon_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                    session_user, holder_acc):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_neon_program_id = Keypair().public_key
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_NEON_PROGRAM, fake_neon_program_id)):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                               [sender_with_tokens.solana_account_address,
                                                session_user.solana_account_address], 1, operator_keypair,
                                               evm_loader_public_key=fake_neon_program_id)

    def test_incorrect_holder_account(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                      session_user):
        fake_holder_acc = Keypair.generate().public_key
        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_PROGRAM_OWNED, fake_holder_acc)):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, fake_holder_acc,
                                               [sender_with_tokens.solana_account_address,
                                                session_user.solana_account_address], 1, operator_keypair)
