import os
import re
import unittest
from subprocess import CompletedProcess
from eth_keys import keys as eth_keys
# from evm_loader.tests.solana_utils import OperatorAccount, WalletAccount, operator1_keypair_path
from evm_loader.tests.solana_utils import OperatorAccount, SplToken, WalletAccount, getBalance, operator1_keypair_path
# from solana_utils import neon_cli, EvmLoader, PublicKey, sha256
from test_acc_storage_states import CONTRACTS_DIR

import solana
from base58 import b58decode
from enum import IntEnum
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/tests")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
evm_loader_id = os.environ.get("EVM_LOADER")
INVALID_NONCE = 'Invalid Ethereum transaction nonce'
INCORRECT_PROGRAM_ID = 'Incorrect Program Id'

NEON_PAYMENT_TO_TREASURE = int(os.environ.get('NEON_PAYMENT_TO_TREASURE',
                                              5000))
NEON_PAYMENT_TO_DEPOSIT = int(os.environ.get('NEON_PAYMENT_TO_DEPOSIT', 5000))

# from base58 import b58decode
# from spl.token.instructions import get_associated_token_address
# from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
# from eth_utils import abi

# evm_loader_id = os.environ.get("EVM_LOADER")

##################
# test_eth_token.py
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from spl.token.constants import TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi
from decimal import Decimal

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/tests")
evm_loader_id = os.environ.get("EVM_LOADER")
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"

ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))


class NeonCliTest(unittest.TestCase):

    @classmethod
    def setUpClass(cls):
        print("\ntest_neon_cli.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet = OperatorAccount(operator1_keypair_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(
            cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller,
         cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(
            PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            cls.loader.airdropNeonTokens(cls.caller_ether, 201)
            print("Done\n")

        print('Account:', cls.acc.public_key(),
              bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->",
              cls.caller, "({})".format(bytes(PublicKey(cls.caller)).hex()))

        (cls.owner_contract, cls.eth_contract,
         cls.contract_code) = cls.loader.deployChecked(
             CONTRACTS_DIR + 'helloWorld.binary', cls.caller, cls.caller_ether)

        print("contract id: ", cls.owner_contract, cls.eth_contract)
        print("code id: ", cls.contract_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(
            collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(
            4, 'little')

        wallet_2 = RandomAccount()
        cls.acc_2 = wallet_2.get_acc()
        print("wallet_2: ", wallet_2.path)

        if getBalance(cls.acc_2.public_key()) == 0:
            tx = client.request_airdrop(cls.acc_2.public_key(), 10 * 10**9)
            confirm_transaction(client, tx['result'])

        # Create ethereum account for user 2 account
        cls.caller_ether_2 = eth_keys.PrivateKey(
            cls.acc_2.secret_key()).public_key.to_canonical_address()
        (cls.caller_2,
         cls.caller_nonce_2) = cls.loader.ether2program(cls.caller_ether_2)

    def create_storage_account(self, seed):
        storage = PublicKey(
            sha256(
                bytes(self.acc.public_key()) + bytes(seed, 'utf8') +
                bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = TransactionWithComputeBudget()
            trx.add(
                createAccountWithSeed(self.acc.public_key(),
                                      self.acc.public_key(), seed, 10**9,
                                      128 * 1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)

        return storage

    def test_command_deposit(self):
        ether_account = eth_keys.PrivateKey(
            os.urandom(32)).public_key.to_address()
        balance_re = re.compile(r"^.*balance:\s+(\d+).*$", flags=re.DOTALL)
        # Place deposit to empty account
        neon_cli().call("deposit 10 {} --evm_loader {}".format(
            ether_account, evm_loader_id))
        # Get account's balance after
        output = neon_cli().call(
            "get-ether-account-data {} --evm_loader {}".format(
                ether_account, evm_loader_id))
        balance = balance_re.match(output)
        self.assertIsNotNone(balance)
        balance = balance.group(1)
        self.assertEqual(balance, '10000000000')
        # Second deposit (to existing account)
        neon_cli().call("deposit 10 {} --evm_loader {}".format(
            ether_account, evm_loader_id))
        # Get account's balance after
        output = neon_cli().call(
            "get-ether-account-data {} --evm_loader {}".format(
                ether_account, evm_loader_id))
        balance = balance_re.match(output)
        self.assertIsNotNone(balance)
        balance = balance.group(1)
        self.assertEqual(balance, '20000000000')

    def test_command_cancel_trx(self):
        """
        neon-cli cancel-trx <STORAGE_ACCOUNT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        # account = self.create_new_account(evm_loader_id)
        # wallet = OperatorAccount(operator1_keypair_path())
        # account = wallet.get_acc()
        # account = WalletAccount()
        # storage_account = PublicKey(
        #     sha256(
        #         bytes(account.public_key()) +
        #         bytes(account[:8].hex(), 'utf8') +
        #         bytes(PublicKey(evm_loader_id))).digest())
        storage_account = self.create_storage_account(self.acc[:8].hex())
        output = neon_cli().call_run(
            f"cancel-trx {storage_account} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        # self.assertEqual(output.returncode, 1)
        self.assert_exit_code(output)

    def test_command_cancel_transfer_to_empty(self):
        empty_account: bytes = eth_keys.PrivateKey(os.urandom(32)).public_key.to_canonical_address()
        (empty_solana_address, _) = self.loader.ether2program(empty_account)

        func_name = abi.function_signature_to_4byte_selector('transferTo(address)')
        input_data = func_name + bytes(12) + empty_account

        with self.assertRaisesRegex(Exception, 'invalid program argument'):
            self.call_partial_signed(input_data, 1 * 10**18, additional_accounts=[AccountMeta(pubkey=PublicKey(empty_solana_address), is_signer=False, is_writable=False)])

        neon_cli().call("cancel-trx --evm_loader {} {}".format(evm_loader_id, self.storage))

    def test_command_create_ether_account(self):
        """
        neon-cli create-ether-account <ether> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        ether_account = self.generate_address()
        output = neon_cli().call_run(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    def test_command_create_program_address(self):
        """
        neon-cli create-program-address <SEED_STRING> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        output_re = re.compile(r"^(\w+\s+\d{1,3})$", flags=re.DOTALL)
        seed_string = self.generate_address()
        output = neon_cli().call_run(
            f"create-program-address {seed_string} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        self.assertTrue(bool(output_re.search(output.stdout)),
                        "The output structure is not 'address nonce'")

    # def test_command_deploy(self):
    #     """
    #     neon-cli deploy <PROGRAM_FILEPATH> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     """
    #     program_filepath = EVM_LOADER_SO
    #     output = neon_cli().call_run(
    #         f"deploy {program_filepath} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     # Solana Client Error
    #     # self.assertEqual(output.returncode, 113)
    #     self.assert_exit_code(output)

    def test_command_emulate(self):
        """
        neon-cli emulate <SENDER> <CONTRACT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        sender = self.create_new_account(evm_loader_id)
        contract = self.generate_address()
        output = neon_cli().call_run(
            f"emulate {sender} {contract} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    def test_command_get_ether_account_data(self):
        """
        neon-cli get-ether-account-data <ether> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        ether_account = self.generate_address()
        neon_cli().call_run(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        output = neon_cli().call_run(
            f"get-ether-account-data {ether_account} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    def test_command_get_storage_at(self):
        """
        neon-cli get-storage-at <contract_id> <index> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        # contract_id = self.create_new_account(evm_loader_id)

        contract_id = self.eth_contract
        # program_id, bytes_result, code_id = EvmLoader().deployChecked(
        #     CONTRACTS_DIR + "EthToken.binary", contract_id, None)
        index = 0
        output = neon_cli().call_run(
            f"get-storage-at {contract_id} {index} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        # self.assertEqual(output.returncode, 101)
        self.assert_exit_code(output)

    def test_command_help(self):
        """
        neon-cli help
        """
        output = neon_cli().call_without_url(f"help create-ether-account")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    # def test_command_migrate_account(self):
    #     """
    #     neon-cli migrate-account <ETHER> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     """
    #     ether_account = self.generate_address()
    #     neon_cli().call_run(
    #         f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
    #     )
    #     output = neon_cli().call_run(
    #         f"migrate-account {ether_account} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     # Solana Client Error
    #     # self.assertEqual(output.returncode, 113)
    #     self.assert_exit_code(output)

    def test_command_neon_elf_params(self):
        """
        neon-cli neon-elf-params --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        """
        output_re = re.compile(r"([\w]+\=\d+)", flags=re.DOTALL)
        output = neon_cli().call_run(
            f"neon-elf-params --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        self.assertTrue(
            bool(output_re.search(output.stdout)),
            "The output structure is not 'NEON_PARAM=numeric_value'")

    # def test_command_update_valids_table(self):
    #     """
    #     neon-cli update-valids-table <contract_id> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     """
    #     contract_id = self.create_new_account(evm_loader_id)
    #     output = neon_cli().call_run(
    #         f"update-valids-table {contract_id} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     # Code account not found
    #     # self.assertEqual(output.returncode, 207)
    #     self.assert_exit_code(output)

    def test_command_version(self):
        """
        neon-cli -V
        """
        output_re = re.compile(r"neon-cli Neon-cli/v[\d\.]+[\w-]+",
                               flags=re.DOTALL)
        output = neon_cli().call_without_url(f"-V")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        self.assertIn('neon-cli', output.stdout,
                      "There is no 'neon-cli' in version")
        self.assertTrue(
            bool(output_re.search(output.stdout)),
            "The output structure is not 'neon-cli Neon-cli/vNNN-alphanumeric'"
        )

    def generate_address(self) -> str:
        return eth_keys.PrivateKey(os.urandom(32)).public_key.to_address()

    def create_new_account(self, evm_loader_id) -> str:
        ether_account = self.generate_address()
        neon_cli().call_run(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        return ether_account

    def assert_exit_code(self, result: CompletedProcess):
        self.assertEqual(result.returncode, 0, "Return code is not 0")


if __name__ == '__main__':
    unittest.main()
