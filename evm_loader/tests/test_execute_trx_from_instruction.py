import random
import string

import pytest
import solana
import eth_abi
from eth_keys import keys as eth_keys
from eth_utils import abi, to_text
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
from spl.token.instructions import get_associated_token_address

from .solana_utils import execute_trx_from_instruction, solana_client, get_neon_balance, neon_cli
from .utils.assert_messages import InstructionAsserts
from .utils.constants import NEON_TOKEN_MINT_ID
from .utils.contract import deploy_contract, make_contract_call_trx
from .utils.ethereum import make_eth_transaction
from .utils.transaction_checks import check_transaction_logs_have_text
from .utils.types import Caller


class TestExecuteTrxFromInstruction:

    def test_simple_transfer_transaction(self, operator_keypair, treasury_pool, sender_with_tokens, session_user,
                                         evm_loader):
        amount = 10
        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_before = get_neon_balance(solana_client, session_user.solana_account_address)

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             session_user.solana_account_address],
                                            operator_keypair)
        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_after = get_neon_balance(solana_client, session_user.solana_account_address)
        assert sender_balance_before - amount == sender_balance_after
        assert recipient_balance_before + amount == recipient_balance_after
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

    def test_transfer_transaction_with_non_existing_recipient(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                              evm_loader):
        # recipient account should be created
        recipient = Keypair.generate()

        recipient_ether = eth_keys.PrivateKey(recipient.secret_key[:32]).public_key.to_canonical_address()
        recipient_solana_address, _ = evm_loader.ether2program(recipient_ether)
        amount = 10
        signed_tx = make_eth_transaction(recipient_ether, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             PublicKey(recipient_solana_address)],
                                            operator_keypair)
        recipient_balance_after = get_neon_balance(solana_client, PublicKey(recipient_solana_address))
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        assert recipient_balance_after == amount

    def test_call_contract_function_without_neon_transfer(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                          evm_loader, string_setter_contract):
        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))
        signed_tx = make_contract_call_trx(sender_with_tokens, string_setter_contract, "set(string)", [text])

        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             string_setter_contract.solana_address],
                                            operator_keypair)

        check_transaction_logs_have_text(resp.value, "exit_status=0x11")
        assert text in to_text(
            neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, string_setter_contract,
                                                  "get()"))

    def test_call_contract_function_with_neon_transfer(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                       evm_loader):
        transfer_amount = random.randint(1, 1000)

        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)

        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_before = get_neon_balance(solana_client, contract.solana_address)

        text = ''.join(random.choice(string.ascii_letters) for i in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, transfer_amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             contract.solana_address],
                                            operator_keypair)

        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        assert text in to_text(neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()"))

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_after = get_neon_balance(solana_client, contract.solana_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

    def test_incorrect_chain_id(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader, session_user):
        amount = 1
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount, chain_id=1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_CHAIN_ID):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_nonce(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader, session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)

        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [sender_with_tokens.solana_account_address,
                                      session_user.solana_account_address],
                                     operator_keypair)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_NONCE):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_insufficient_funds(self, operator_keypair, treasury_pool, evm_loader, sender_with_tokens, session_user):
        user_balance = get_neon_balance(solana_client, session_user.solana_account_address)

        signed_tx = make_eth_transaction(sender_with_tokens.eth_address, None, session_user.solana_account,
                                         session_user.solana_account_address, user_balance + 1)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INSUFFICIENT_FUNDS):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_gas_limit_reached(self, operator_keypair, treasury_pool, session_user, evm_loader, sender_with_tokens):
        amount = 10
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount, gas=1)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.OUT_OF_GAS):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [session_user.solana_account_address,
                                          sender_with_tokens.solana_account_address],
                                         operator_keypair)

    def test_sender_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, session_user,
                                                 sender_with_tokens, evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [session_user.solana_account_address],
                                         operator_keypair)

    def test_recipient_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                    evm_loader, session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address],
                                         operator_keypair)

    def test_incorrect_treasure_pool(self, operator_keypair, sender_with_tokens, evm_loader, session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        treasury_buffer = b'\x02\x00\x00\x00'
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_TREASURE_ACC):
            execute_trx_from_instruction(operator_keypair, evm_loader, Keypair().public_key, treasury_buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_treasure_index(self, operator_keypair, sender_with_tokens, evm_loader, treasury_pool,
                                      session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        treasury_buffer = b'\x03\x00\x00\x00'
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_TREASURE_ACC):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_operator_account(self, sender_with_tokens, evm_loader, treasury_pool, session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_operator = Keypair()
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ACC_NOT_FOUND):
            execute_trx_from_instruction(fake_operator, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         fake_operator)

    def test_operator_is_not_in_white_list(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                           session_user):
        # now any user can send transactions through "execute transaction from instruction" instruction

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        resp = execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, treasury_pool.account,
                                            treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             session_user.solana_account_address],
                                            sender_with_tokens.solana_account)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

    def test_incorrect_system_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                      session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_sys_program_id = Keypair().public_key
        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_SYSTEM_PROGRAM, fake_sys_program_id)):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair, system_program=fake_sys_program_id)

    def test_incorrect_neon_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                    session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_neon_program_id = Keypair().public_key
        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_NEON_PROGRAM, fake_neon_program_id)):
            execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         sender_with_tokens.solana_account, evm_loader_public_key=fake_neon_program_id)

    def test_operator_does_not_have_enough_founds(self, sender_with_tokens, evm_loader, treasury_pool,
                                                  session_user):
        key = Keypair.generate()
        caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
        caller, caller_nonce = evm_loader.ether2program(caller_ether)
        caller_token = get_associated_token_address(PublicKey(caller), NEON_TOKEN_MINT_ID)
        evm_loader.create_ether_account(caller_ether)

        operator_without_money = Caller(key, PublicKey(caller), caller_ether, caller_nonce, caller_token)

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException,
                           match="Attempt to debit an account but found no record of a prior credit"):
            execute_trx_from_instruction(operator_without_money.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          session_user.solana_account_address],
                                         operator_without_money.solana_account)
