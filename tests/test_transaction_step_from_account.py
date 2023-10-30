import random
import string
import time

import eth_abi
import pytest
import solana
from eth_keys import keys as eth_keys
from eth_utils import abi, to_text, to_int
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Processed, Confirmed
from solana.rpc.types import TxOpts

from .solana_utils import get_neon_balance, solana_client, neon_cli, execute_transaction_steps_from_account, \
    write_transaction_to_holder_account, create_treasury_pool_address, send_transaction_step_from_account
from .test_cli import gen_hash_of_block
from .utils.assert_messages import InstructionAsserts
from .utils.constants import TAG_FINALIZED_STATE
from .utils.contract import make_deployment_transaction, make_contract_call_trx, deploy_contract
from .utils.ethereum import make_eth_transaction, create_contract_address
from .utils.instructions import TransactionWithComputeBudget, make_ExecuteTrxFromAccountDataIterativeOrContinue
from .utils.layouts import FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder
from .utils.transaction_checks import check_transaction_logs_have_text, check_holder_account_tag
from .utils.types import TreasuryPool


def generate_access_lists():
    addr1 = gen_hash_of_block(20)
    addr2 = gen_hash_of_block(20)
    key1, key2, key3, key4 = (f"0x000000000000000000000000000000000000000000000000000000000000000{item}" for item in
                              (0, 1, 2, 3))
    return (({"address": addr1, "storageKeys": []},),
            ({"address": addr1, "storageKeys": (key1, key2, key3, key4)},),
            ({"address": addr1, "storageKeys": (key1, key2)}, {"address": addr2, "storageKeys": []}),
            ({"address": addr1, "storageKeys": (key1, key2)}, {"address": addr2, "storageKeys": (key3,)}))


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

    def test_deploy_contract(self, operator_keypair, holder_acc, treasury_pool, evm_loader, sender_with_tokens, is_eof):
        contract_filename = "hello_world.binary"
        contract = create_contract_address(sender_with_tokens, evm_loader)

        signed_tx = make_deployment_transaction(sender_with_tokens, contract_filename, is_eof=is_eof)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        contract_path = (pytest.EOF_CONTRACTS_PATH if is_eof else pytest.CONTRACTS_PATH) / contract_filename
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
                                                          sender_with_tokens, evm_loader, string_setter_contract_with_eof):
        string_setter_contract, is_eof = string_setter_contract_with_eof
        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))
        signed_tx = make_contract_call_trx(sender_with_tokens, string_setter_contract, "set(string)", [text])
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [string_setter_contract.solana_address,
                                                       sender_with_tokens.solana_account_address])

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12" if is_eof else "exit_status=0x11")

        assert text in to_text(
            neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, string_setter_contract,
                                                  "get()"))

    def test_call_contract_function_with_neon_transfer(self, operator_keypair, treasury_pool,
                                                       sender_with_tokens, string_setter_contract_with_eof, holder_acc,
                                                       evm_loader):
        string_setter_contract, is_eof = string_setter_contract_with_eof
        transfer_amount = random.randint(1, 1000)

        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_before = get_neon_balance(solana_client, string_setter_contract.solana_address)

        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))

        signed_tx = make_contract_call_trx(sender_with_tokens, string_setter_contract, "set(string)", [text],
                                           value=transfer_amount)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [string_setter_contract.solana_address,
                                                       sender_with_tokens.solana_account_address]
                                                      )

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12" if is_eof else "exit_status=0x11")

        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_after = get_neon_balance(solana_client, string_setter_contract.solana_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

        assert text in to_text(
            neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, string_setter_contract,
                                                  "get()"))

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
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

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
                                holder_acc, sender_with_tokens):
        user_balance = get_neon_balance(solana_client, session_user.solana_account_address)

        signed_tx = make_eth_transaction(sender_with_tokens.eth_address, None, session_user.solana_account,
                                         session_user.solana_account_address, user_balance + 1)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.INSUFFICIENT_FUNDS):
            execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                   [session_user.solana_account_address,
                                                    sender_with_tokens.solana_account_address], 0)

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

    def test_transaction_with_access_list(self, operator_keypair, holder_acc, treasury_pool,
                                          sender_with_tokens, evm_loader, calculator_contract,
                                          calculator_caller_contract):
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
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [calculator_caller_contract.solana_address,
                                                       calculator_contract.solana_address,
                                                       sender_with_tokens.solana_account_address])

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12")

    @pytest.mark.parametrize("access_list", generate_access_lists())
    def test_access_list_structure(self, operator_keypair, holder_acc, treasury_pool, evm_loader,
                                   sender_with_tokens, string_setter_contract, access_list):
        text = ''.join(random.choice(string.ascii_letters) for _ in range(10))

        signed_tx = make_contract_call_trx(sender_with_tokens, string_setter_contract, "set(string)", [text],
                                           value=10, access_list=access_list)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [string_setter_contract.solana_address,
                                                       sender_with_tokens.solana_account_address])

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

        assert text in to_text(
            neon_cli().call_contract_get_function(evm_loader, sender_with_tokens, string_setter_contract,
                                                  "get()"))


class TestAccountStepContractCallContractInteractions:
    def test_contract_call_unchange_storage_function(self, rw_lock_contract, rw_lock_caller, session_user, evm_loader,
                                                     operator_keypair, treasury_pool, holder_acc):
        signed_tx = make_contract_call_trx(session_user, rw_lock_caller, 'unchange_storage(uint8,uint8)', [1, 1])
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [rw_lock_caller.solana_address,
                                                       rw_lock_contract.solana_address,
                                                       session_user.solana_account_address])

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12")

    def test_contract_call_set_function(self, rw_lock_contract, session_user, evm_loader, operator_keypair,
                                        treasury_pool, holder_acc, rw_lock_caller):
        signed_tx = make_contract_call_trx(session_user, rw_lock_caller, 'update_storage_str(string)', ['hello'])
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [rw_lock_caller.solana_address,
                                                       rw_lock_contract.solana_address,
                                                       session_user.solana_account_address], 1000)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

        assert 'hello' in to_text(neon_cli().call_contract_get_function(evm_loader, session_user, rw_lock_contract,
                                                                        "get_text()"))

    def test_contract_call_get_function(self, rw_lock_contract, session_user, evm_loader, operator_keypair,
                                        treasury_pool, holder_acc, rw_lock_caller):
        signed_tx = make_contract_call_trx(session_user, rw_lock_caller, 'get_text()')
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      [rw_lock_caller.solana_address,
                                                       rw_lock_contract.solana_address,
                                                       session_user.solana_account_address], 1000)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12")

    def test_contract_call_update_storage_map_function(self, rw_lock_contract, session_user, evm_loader,
                                                       operator_keypair, rw_lock_caller,
                                                       treasury_pool, holder_acc):
        signed_tx = make_contract_call_trx(session_user, rw_lock_caller, 'update_storage_map(uint256)', [3])
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)

        func_name = abi.function_signature_to_4byte_selector('update_storage_map(uint256)')
        data = func_name + eth_abi.encode(['uint256'], [3])
        result = neon_cli().emulate(evm_loader.loader_id,
                                    session_user.eth_address.hex(),
                                    rw_lock_caller.eth_address.hex(),
                                    data.hex())
        additional_accounts = [session_user.solana_account_address, rw_lock_contract.solana_address,
                               rw_lock_caller.solana_address]
        for acc in result['solana_accounts']:
            additional_accounts.append(PublicKey(acc['pubkey']))

        resp = execute_transaction_steps_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc,
                                                      additional_accounts)

        check_holder_account_tag(holder_acc, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x11")

        constructor_args = eth_abi.encode(['address', 'uint256'], [rw_lock_caller.eth_address.hex(), 2])
        actual_data = neon_cli().call_contract_get_function(evm_loader, session_user, rw_lock_contract,
                                                            "data(address,uint256)", constructor_args)
        assert to_int(hexstr=actual_data) == 2, "Contract data is not correct"


class TestTransactionStepFromAccountParallelRuns:

    def test_one_user_call_2_contracts(self, rw_lock_contract, string_setter_contract, user_account, evm_loader,
                                       operator_keypair, treasury_pool, new_holder_acc):
        signed_tx = make_contract_call_trx(user_account, rw_lock_contract, 'unchange_storage(uint8,uint8)', [1, 1])
        write_transaction_to_holder_account(signed_tx, new_holder_acc, operator_keypair)

        send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                           [user_account.solana_account_address,
                                            rw_lock_contract.solana_address], 1, operator_keypair)

        signed_tx2 = make_contract_call_trx(user_account, string_setter_contract, 'get()')

        holder_acc2 = create_holder(operator_keypair)
        write_transaction_to_holder_account(signed_tx2, holder_acc2, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.LOCKED_ACC):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc2,
                                               [user_account.solana_account_address,
                                                string_setter_contract.solana_address], 1, operator_keypair)

    def test_2_users_call_the_same_contract(self, rw_lock_contract, user_account,
                                            session_user, evm_loader, operator_keypair,
                                            treasury_pool, new_holder_acc):
        signed_tx = make_contract_call_trx(user_account, rw_lock_contract, 'unchange_storage(uint8,uint8)', [1, 1])
        write_transaction_to_holder_account(signed_tx, new_holder_acc, operator_keypair)

        send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                           [user_account.solana_account_address,
                                            rw_lock_contract.solana_address], 1, operator_keypair)

        signed_tx2 = make_contract_call_trx(session_user, rw_lock_contract, 'get_text()')

        holder_acc2 = create_holder(operator_keypair)
        write_transaction_to_holder_account(signed_tx2, holder_acc2, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.LOCKED_ACC):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc2,
                                               [session_user.solana_account_address,
                                                rw_lock_contract.solana_address], 1, operator_keypair)

    def test_two_contracts_call_same_contract(self, rw_lock_contract, user_account,
                                              session_user, evm_loader, operator_keypair,
                                              treasury_pool, new_holder_acc):
        constructor_args = eth_abi.encode(['address'], [rw_lock_contract.eth_address.hex()])

        contract1 = deploy_contract(operator_keypair, session_user, "rw_lock_caller.binary", evm_loader, treasury_pool,
                                    encoded_args=constructor_args)
        contract2 = deploy_contract(operator_keypair, session_user, "rw_lock_caller.binary", evm_loader, treasury_pool,
                                    encoded_args=constructor_args)

        signed_tx1 = make_contract_call_trx(user_account, contract1, 'unchange_storage(uint8,uint8)', [1, 1])
        signed_tx2 = make_contract_call_trx(session_user, contract2, 'get_text()')
        write_transaction_to_holder_account(signed_tx1, new_holder_acc, operator_keypair)

        send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                           [user_account.solana_account_address,
                                            rw_lock_contract.solana_address,
                                            contract1.solana_address], 1, operator_keypair)

        holder_acc2 = create_holder(operator_keypair)
        write_transaction_to_holder_account(signed_tx2, holder_acc2, operator_keypair)

        with pytest.raises(solana.rpc.core.RPCException, match=InstructionAsserts.LOCKED_ACC):
            send_transaction_step_from_account(operator_keypair, evm_loader, treasury_pool, holder_acc2,
                                               [session_user.solana_account_address,
                                                rw_lock_contract.solana_address,
                                                contract2.solana_address], 1,
                                               operator_keypair)


class TestStepFromAccountChangingOperatorsDuringTrxRun:
    def test_next_operator_can_continue_trx_after_some_time(self, rw_lock_contract_with_eof, user_account, evm_loader,
                                                            operator_keypair, second_operator_keypair, treasury_pool,
                                                            new_holder_acc):
        rw_lock_contract, is_eof = rw_lock_contract_with_eof
        signed_tx = make_contract_call_trx(user_account, rw_lock_contract, 'update_storage_str(string)', ['text'])
        write_transaction_to_holder_account(signed_tx, new_holder_acc, operator_keypair)

        trx = TransactionWithComputeBudget(operator_keypair)
        trx.add(
            make_ExecuteTrxFromAccountDataIterativeOrContinue(
                operator_keypair, evm_loader, new_holder_acc, treasury_pool.account, treasury_pool.buffer, 1,
                [user_account.solana_account_address,
                 rw_lock_contract.solana_address]
            )
        )
        solana_client.send_transaction(trx, operator_keypair,
                                       opts=TxOpts(skip_confirmation=True, preflight_commitment=Confirmed))

        # next operator can't continue trx during OPERATOR_PRIORITY_SLOTS*0.4
        with pytest.raises(solana.rpc.core.RPCException,
                           match=rf"{InstructionAsserts.INVALID_OPERATOR_KEY}|{InstructionAsserts.INVALID_HOLDER_OWNER}"):            send_transaction_step_from_account(
            second_operator_keypair, evm_loader, treasury_pool, new_holder_acc,
            [user_account.solana_account_address,
             rw_lock_contract.solana_address], 500, second_operator_keypair)

        time.sleep(15)
        send_transaction_step_from_account(second_operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                           [user_account.solana_account_address,
                                            rw_lock_contract.solana_address], 500, second_operator_keypair)
        resp = send_transaction_step_from_account(second_operator_keypair, evm_loader, treasury_pool, new_holder_acc,
                                                  [user_account.solana_account_address,
                                                   rw_lock_contract.solana_address], 1, second_operator_keypair)
        check_transaction_logs_have_text(resp.value.transaction.transaction.signatures[0], "exit_status=0x12" if is_eof else "exit_status=0x11")
