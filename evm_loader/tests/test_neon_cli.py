import os
import re
from subprocess import CompletedProcess
from unittest import TestCase
from eth_keys import keys as eth_keys
# from evm_loader.tests.solana_utils import OperatorAccount, WalletAccount, operator1_keypair_path
from evm_loader.tests.solana_utils import OperatorAccount, SplToken, WalletAccount, getBalance, operator1_keypair_path
from solana_utils import neon_cli, EvmLoader, PublicKey, sha256
from test_acc_storage_states import CONTRACTS_DIR

# from base58 import b58decode
# from spl.token.instructions import get_associated_token_address
# from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
# from eth_utils import abi

evm_loader_id = os.environ.get("EVM_LOADER")


class NeonCliTest(TestCase):

    @classmethod
    def setUpClass(cls):
        print("\ntest_neon_cli.py setUpClass")

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

    # def test_command_cancel_trx(self):
    #     """
    #     neon-cli cancel-trx <STORAGE_ACCOUNT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     """
    #     # account = self.create_new_account(evm_loader_id)
    #     # wallet = OperatorAccount(operator1_keypair_path())
    #     # account = wallet.get_acc()
    #     account = WalletAccount()
    #     storage_account = PublicKey(
    #         sha256(
    #             bytes(account.public_key()) +
    #             bytes(account[:8].hex(), 'utf8') +
    #             bytes(PublicKey(evm_loader_id))).digest())
    #     output = neon_cli().call_run(
    #         f"cancel-trx {storage_account} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     # self.assertEqual(output.returncode, 1)
    #     self.assert_exit_code(output)

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

        # token = SplToken(solana_url)
        wallet = OperatorAccount(operator1_keypair_path())
        loader = EvmLoader(wallet, evm_loader_id)
        acc = wallet.get_acc()

        # Create ethereum account for user account
        caller_ether = eth_keys.PrivateKey(
            acc.secret_key()).public_key.to_canonical_address()
        (caller, caller_nonce) = loader.ether2program(caller_ether)
        # caller_token = get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID)

        if getBalance(caller) == 0:
            print("Create caller account...")
            _ = loader.createEtherAccount(caller_ether)
            loader.airdropNeonTokens(caller_ether, 201)
            print("Done\n")

        print('Account:', acc.public_key(), bytes(acc.public_key()).hex())
        print("Caller:", caller_ether.hex(), caller_nonce, "->", caller,
              "({})".format(bytes(PublicKey(caller)).hex()))

        (owner_contract, eth_contract, contract_code) = loader.deployChecked(
            CONTRACTS_DIR + 'helloWorld.binary', caller, caller_ether)
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
