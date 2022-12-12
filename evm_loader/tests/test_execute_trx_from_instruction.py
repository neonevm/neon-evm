import base64
import json
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

from .solana_utils import execute_trx_from_instruction, deposit_neon, solana_client, get_neon_balance, \
    neon_cli, make_new_user
from .utils.constants import NEON_TOKEN_MINT_ID
from .utils.contract import make_deployment_transaction, deploy_contract
from .utils.ethereum import make_eth_transaction, create_contract_address
from .utils.types import Caller


class TestExecuteTrxFromInstruction:

    @pytest.fixture(scope="session")
    def sender_with_tokens(self, evm_loader, operator_keypair):
        user = make_new_user(evm_loader)
        deposit_neon(evm_loader, operator_keypair, user.eth_address, 100000)
        return user

    def check_transaction_logs_have_text(self, trx_hash, text):
        receipt = solana_client.get_transaction(trx_hash)
        logs = ""
        for log in receipt.value.transaction.meta.log_messages:
            if "Program data:" in log:
                logs += "Program data: " + str(base64.b64decode(log.replace("Program data: ", "")))
            else:
                logs += log
            logs += " "
        assert text in logs, f"Transaction logs don't contain '{text}'. Logs: {logs}"

    def test_simple_transfer_transaction(self, operator_keypair, treasury_pool, sender_with_tokens, second_user,
                                         evm_loader):
        amount = 10
        sender_balance_before = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_before = get_neon_balance(solana_client, second_user.solana_account_address)

        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             second_user.solana_account_address],
                                            operator_keypair)
        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        recipient_balance_after = get_neon_balance(solana_client, second_user.solana_account_address)
        assert sender_balance_before - amount == sender_balance_after
        assert recipient_balance_before + amount == recipient_balance_after
        self.check_transaction_logs_have_text(resp.value, "ExitSucceed")

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
        self.check_transaction_logs_have_text(resp.value, "ExitSucceed")

        assert recipient_balance_after == amount

    def test_deploy_contract(self, operator_keypair, evm_loader, treasury_pool, sender_with_tokens):
        contract_filename = "small.binary"

        signed_tx = make_deployment_transaction(sender_with_tokens, contract_filename)
        contract = create_contract_address(sender_with_tokens, evm_loader)

        with pytest.raises(solana.rpc.core.RPCException, match="Deploy transactions are not allowed"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          contract.solana_address],
                                         operator_keypair)

    @pytest.mark.parametrize("chain_id", [None, 111])
    def test_call_contract_function_without_neon_transfer(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                          evm_loader, chain_id):
        contract = deploy_contract(operator_keypair, sender_with_tokens, "string_setter.binary", evm_loader,
                                   treasury_pool)
        text = ''.join(random.choice(string.ascii_letters) for i in range(10))
        func_name = abi.function_signature_to_4byte_selector('set(string)')
        data = func_name + eth_abi.encode(['string'], [text])
        signed_tx = make_eth_transaction(contract.eth_address, data, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, chain_id=chain_id)
        resp = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             contract.solana_address],
                                            operator_keypair)

        self.check_transaction_logs_have_text(resp.value, "ExitSucceed")

        data = abi.function_signature_to_4byte_selector('get()')
        result = json.loads(
            neon_cli().emulate(evm_loader.loader_id, sender_with_tokens.eth_address.hex(), contract.eth_address.hex(),
                               data.hex())
        )
        assert text in to_text(result["result"])

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

        self.check_transaction_logs_have_text(resp.value, "ExitSucceed")

        data = abi.function_signature_to_4byte_selector('get()')
        result = json.loads(
            neon_cli().emulate(evm_loader.loader_id, sender_with_tokens.eth_address.hex(), contract.eth_address.hex(),
                               data.hex())
        )
        assert text in to_text(result["result"])
        sender_balance_after = get_neon_balance(solana_client, sender_with_tokens.solana_account_address)
        contract_balance_after = get_neon_balance(solana_client, contract.solana_address)
        assert sender_balance_before - transfer_amount == sender_balance_after
        assert contract_balance_before + transfer_amount == contract_balance_after

    def test_incorrect_chain_id(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader, second_user):
        amount = 1
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount, chain_id=1)
        with pytest.raises(solana.rpc.core.RPCException, match="Invalid chain_id"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_nonce(self, operator_keypair, treasury_pool, sender_with_tokens, evm_loader, second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)

        execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                     signed_tx,
                                     [sender_with_tokens.solana_account_address,
                                      second_user.solana_account_address],
                                     operator_keypair)
        with pytest.raises(solana.rpc.core.RPCException, match="Invalid Ethereum transaction nonce"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_keypair)

    def test_insufficient_funds(self, operator_keypair, treasury_pool, user_account, evm_loader, second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, user_account.solana_account,
                                         user_account.solana_account_address, 10)

        with pytest.raises(solana.rpc.core.RPCException, match="insufficient funds for instruction"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [user_account.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_keypair)

    def test_gas_limit_reached(self, operator_keypair, treasury_pool, second_user, evm_loader, sender_with_tokens):
        amount = 10
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, amount, gas=1)

        with pytest.raises(solana.rpc.core.RPCException, match="Out of gas used"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [second_user.solana_account_address,
                                          sender_with_tokens.solana_account_address],
                                         operator_keypair)

    def test_sender_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, user_account,
                                                 sender_with_tokens, evm_loader):
        signed_tx = make_eth_transaction(sender_with_tokens.eth_address, None, user_account.solana_account,
                                         user_account.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=r"address .* must be present in the transaction"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address],
                                         operator_keypair)

    def test_recipient_missed_in_remaining_accounts(self, operator_keypair, treasury_pool, sender_with_tokens,
                                                    evm_loader,
                                                    second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException, match=r"address .* must be present in the transaction"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address],
                                         operator_keypair)

    def test_incorrect_treasure_pool(self, operator_keypair, sender_with_tokens, evm_loader, second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        treasury_buffer = b'\x02\x00\x00\x00'
        with pytest.raises(solana.rpc.core.RPCException, match="invalid treasure account"):
            execute_trx_from_instruction(operator_keypair, evm_loader, Keypair().public_key, treasury_buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_treasure_index(self, operator_keypair, sender_with_tokens, evm_loader, treasury_pool,
                                      second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        treasury_buffer = b'\x03\x00\x00\x00'
        with pytest.raises(solana.rpc.core.RPCException, match="invalid treasure account"):
            execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_keypair)

    def test_incorrect_operator_account(self, sender_with_tokens, evm_loader, treasury_pool, second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_operator = Keypair()
        with pytest.raises(solana.rpc.core.RPCException, match="AccountNotFound"):
            execute_trx_from_instruction(fake_operator, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         fake_operator)

    def test_operator_is_not_in_white_list(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                           second_user):
        # now any user can send transactions through "execute transaction from instruction" instruction

        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        resp = execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, treasury_pool.account,
                                            treasury_pool.buffer,
                                            signed_tx,
                                            [sender_with_tokens.solana_account_address,
                                             second_user.solana_account_address],
                                            sender_with_tokens.solana_account)
        self.check_transaction_logs_have_text(resp.value, "ExitSucceed")

    def test_incorrect_system_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool,
                                      second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_sys_program_id = Keypair().public_key
        with pytest.raises(solana.rpc.core.RPCException,
                           match=f"Account {fake_sys_program_id} - is not system program"):
            execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         sender_with_tokens.solana_account, system_program=fake_sys_program_id)

    def test_incorrect_neon_program(self, sender_with_tokens, operator_keypair, evm_loader, treasury_pool, second_user):
        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        fake_neon_program_id = Keypair().public_key
        with pytest.raises(solana.rpc.core.RPCException, match=f"Account {fake_neon_program_id} - is not Neon program"):
            execute_trx_from_instruction(sender_with_tokens.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         sender_with_tokens.solana_account, evm_loader_public_key=fake_neon_program_id)

    def test_operator_does_not_have_enough_founds(self, operator_keypair, sender_with_tokens, evm_loader, treasury_pool,
                                                  second_user):
        key = Keypair.generate()
        caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
        caller, caller_nonce = evm_loader.ether2program(caller_ether)
        caller_token = get_associated_token_address(PublicKey(caller), NEON_TOKEN_MINT_ID)
        evm_loader.create_ether_account(caller_ether)

        operator_without_money = Caller(key, PublicKey(caller), caller_ether, caller_nonce, caller_token)

        signed_tx = make_eth_transaction(second_user.eth_address, None, sender_with_tokens.solana_account,
                                         sender_with_tokens.solana_account_address, 1)
        with pytest.raises(solana.rpc.core.RPCException,
                           match="Attempt to debit an account but found no record of a prior credit"):
            execute_trx_from_instruction(operator_without_money.solana_account, evm_loader, treasury_pool.account,
                                         treasury_pool.buffer,
                                         signed_tx,
                                         [sender_with_tokens.solana_account_address,
                                          second_user.solana_account_address],
                                         operator_without_money.solana_account)
