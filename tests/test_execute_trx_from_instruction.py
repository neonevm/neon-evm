import random
import string

import pytest
import solana
import eth_abi
from eth_account.datastructures import SignedTransaction
from eth_keys import keys as eth_keys
from eth_utils import abi, to_text
from hexbytes import HexBytes
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
from spl.token.instructions import get_associated_token_address

from .solana_utils import execute_trx_from_instruction, solana_client, neon_cli
from .utils.assert_messages import InstructionAsserts
from .utils.constants import NEON_TOKEN_MINT_ID
from .utils.contract import deploy_contract, make_contract_call_trx
from .utils.ethereum import make_eth_transaction
from .utils.transaction_checks import check_transaction_logs_have_text
from .utils.types import Caller, Contract


class TestExecuteTrxFromInstruction:

    def test_simple_transfer_transaction(self, operator_keypair, treasury_pool, 
                                         sender_with_tokens: Caller, session_user: Caller,
                                         evm_loader):
        amount = 10
        sender_balance_before = evm_loader.get_neon_balance(sender_with_tokens)
        recipient_balance_before = evm_loader.get_neon_balance(session_user)

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                            session_user.balance_account_address,
                                            session_user.solana_account_address],
                                            operator_keypair)
        sender_balance_after = evm_loader.get_neon_balance(sender_with_tokens)
        recipient_balance_after = evm_loader.get_neon_balance(session_user)
        assert sender_balance_before - amount == sender_balance_after
        assert recipient_balance_before + amount == recipient_balance_after
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

    def test_transfer_transaction_with_non_existing_recipient(self, operator_keypair, treasury_pool, 
                                                              sender_with_tokens: Caller,
                                                              evm_loader):
        # recipient account should be created
        recipient = Keypair.generate()

        recipient_ether = eth_keys.PrivateKey(recipient.secret_key[:32]).public_key.to_canonical_address()
        recipient_solana_address, _ = evm_loader.ether2program(recipient_ether)
        recipient_balance_address = evm_loader.ether2balance(recipient_ether)
        amount = 10
        signed_tx = make_eth_transaction(recipient_ether, None, sender_with_tokens, amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                             recipient_balance_address,
                                             PublicKey(recipient_solana_address)],
                                            operator_keypair)

        recipient_balance_after = evm_loader.get_neon_balance(recipient_ether)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        assert recipient_balance_after == amount

    def test_call_contract_function_without_neon_transfer(self, operator_keypair, treasury_pool, 
                                                          sender_with_tokens: Caller, string_setter_contract: Contract,
                                                          evm_loader):
        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))
        signed_tx = make_contract_call_trx(sender_with_tokens, string_setter_contract, "set(string)", [text])

        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                             string_setter_contract.solana_address],
                                            operator_keypair)

        check_transaction_logs_have_text(resp.value, "exit_status=0x11")
        assert text in to_text(
            neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, string_setter_contract,
                                                  "get()"))

    def test_call_contract_function_with_neon_transfer(self, operator_keypair, treasury_pool, 
                                                       sender_with_tokens: Caller,
                                                       evm_loader):
        transfer_amount = random.randint(1, 1000)

        contract: Contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)

        sender_balance_before = evm_loader.get_neon_balance(sender_with_tokens)
        contract_balance_before = evm_loader.get_neon_balance(contract.eth_address)

        text = ''.join(random.choice(string.ascii_letters) for i in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens, transfer_amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                             contract.balance_account_address,
                                             contract.solana_address],
                                            operator_keypair)

        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

        assert text in to_text(neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, contract, "get()"))

        sender_balance_after = evm_loader.get_neon_balance(sender_with_tokens)
        contract_balance_after = evm_loader.get_neon_balance(contract.eth_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

    def test_incorrect_chain_id(self, operator_keypair, treasury_pool, 
                                sender_with_tokens: Caller, session_user: Caller,
                                evm_loader):
        amount = 1
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, amount, chain_id=1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_CHAIN_ID):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address,
                                          session_user.balance_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_nonce(self, operator_keypair, treasury_pool, 
                             sender_with_tokens: Caller, session_user: Caller, 
                             evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)

        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [sender_with_tokens.balance_account_address,
                                      session_user.balance_account_address,
                                      session_user.solana_account_address],
                                     operator_keypair)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INVALID_NONCE):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                        [sender_with_tokens.balance_account_address,
                                        session_user.balance_account_address,
                                        session_user.solana_account_address],
                                         operator_keypair)

    def test_insufficient_funds(self, operator_keypair, treasury_pool, evm_loader, 
                                sender_with_tokens: Caller, session_user: Caller):
        user_balance = evm_loader.get_neon_balance(session_user)

        signed_tx = make_eth_transaction(sender_with_tokens.eth_address, None, session_user, user_balance + 1)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INSUFFICIENT_FUNDS):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address,
                                          session_user.balance_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_gas_limit_reached(self, operator_keypair, treasury_pool, 
                               session_user: Caller, sender_with_tokens: Caller,
                               evm_loader):
        amount = 10
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, amount, gas=1)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.OUT_OF_GAS):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address,
                                          session_user.balance_account_address,
                                          session_user.solana_account_address],
                                         operator_keypair)

    def test_sender_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, 
                                                 session_user: Caller, sender_with_tokens: Caller, 
                                                 evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [session_user.balance_account_address,
                                         session_user.solana_account_address],
                                         operator_keypair)

    def test_recipient_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, 
                                                    sender_with_tokens: Caller, session_user: Caller,
                                                    evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ADDRESS_MUST_BE_PRESENT):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address],
                                         operator_keypair)

    def test_incorrect_treasure_pool(self, operator_keypair, 
                                     sender_with_tokens: Caller, session_user: Caller,
                                     evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)

        treasury_buffer = b'\x02\x00\x00\x00'
        treasury_pool = Keypair().public_key

        error = str.format(InstructionAsserts.INVALID_ACCOUNT, treasury_pool)
        with pytest.raises(solana.rpc.core.RPCException, match=error):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool, treasury_buffer,
                                         signed_tx,
                                         [],
                                         operator_keypair)

    def test_incorrect_treasure_index(self, operator_keypair, treasury_pool,
                                      sender_with_tokens: Caller, session_user: Caller,
                                      evm_loader):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        treasury_buffer = b'\x03\x00\x00\x00'

        error = str.format(InstructionAsserts.INVALID_ACCOUNT, treasury_pool.account)
        with pytest.raises(solana.rpc.core.RPCException, match=error):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_buffer,
                                         signed_tx,
                                         [],
                                         operator_keypair)

    def test_incorrect_operator_account(self, evm_loader, treasury_pool,
                                         session_user: Caller, sender_with_tokens: Caller):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        fake_operator = Keypair()
        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.ACC_NOT_FOUND):
            execute_trx_from_instruction(fake_operator, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address,
                                          session_user.balance_account_address,
                                          session_user.solana_account_address],
                                         fake_operator)

    def test_operator_is_not_in_white_list(self, sender_with_tokens, evm_loader, treasury_pool,
                                           session_user):
        # now any user can send transactions through "execute transaction from instruction" instruction

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        resp = execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, 
                                            treasury_pool.account,
                                            treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                             session_user.balance_account_address,
                                             session_user.solana_account_address],
                                            sender_with_tokens.solana_account)
        check_transaction_logs_have_text(resp.value, "exit_status=0x11")

    def test_incorrect_system_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                      session_user):
        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        fake_sys_program_id = Keypair().public_key
        with pytest.raises(solana.rpc.core.RPCException,
                           match=str.format(InstructionAsserts.NOT_SYSTEM_PROGRAM, fake_sys_program_id)):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [],
                                         operator_keypair, system_program=fake_sys_program_id)

    def test_operator_does_not_have_enough_founds(self, evm_loader, treasury_pool,
                                                  session_user: Caller, sender_with_tokens: Caller):
        key = Keypair.generate()
        caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
        caller, caller_nonce = evm_loader.ether2program(caller_ether)
        caller_token = get_associated_token_address(PublicKey(caller), NEON_TOKEN_MINT_ID)
        evm_loader.create_balance_account(caller_ether)

        operator_without_money = Caller(key, PublicKey(caller), caller_ether, caller_nonce, caller_token)

        signed_tx = make_eth_transaction(session_user.eth_address, None, sender_with_tokens, 1)
        with pytest.raises(solana.rpc.core.RPCException,
                           match="Attempt to debit an account but found no record of a prior credit"):
            execute_trx_from_instruction(operator_without_money.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.balance_account_address,
                                          session_user.balance_account_address,
                                          session_user.solana_account_address],
                                         operator_without_money.solana_account)

    def test_transaction_with_access_list(self, operator_keypair, treasury_pool, sender_with_tokens,
                                          evm_loader, calculator_contract, calculator_caller_contract):
        access_list = (
            {
                "address": '0x' + calculator_contract.eth_address.hex(),
                "storageKeys": (
                    "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "0x0000000000000000000000000000000000000000000000000000000000000001",
                )
            },
        )
        signed_tx = make_contract_call_trx(sender_with_tokens, calculator_caller_contract, "callCalculator()", [],
                                           access_list=access_list)

        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.balance_account_address,
                                             calculator_caller_contract.solana_address,
                                             calculator_contract.solana_address],
                                            operator_keypair)

        check_transaction_logs_have_text(resp.value, "exit_status=0x12")

    def test_old_trx_type_with_leading_zeros(self, sender_with_tokens, operator_keypair, evm_loader,
                                             calculator_caller_contract, calculator_contract, treasury_pool):
        signed_tx = make_contract_call_trx(sender_with_tokens, calculator_caller_contract, "callCalculator()", [])
        new_raw_trx = HexBytes(bytes([0]) + signed_tx.rawTransaction)

        signed_tx_new = SignedTransaction(
            rawTransaction=new_raw_trx,
            hash=signed_tx.hash,
            r=signed_tx.r,
            s=signed_tx.s,
            v=signed_tx.v,
        )

        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx_new,
                                            [sender_with_tokens.balance_account_address,
                                             calculator_caller_contract.solana_address,
                                             calculator_contract.solana_address],
                                            operator_keypair)
        check_transaction_logs_have_text(resp.value, "exit_status=0x12")
