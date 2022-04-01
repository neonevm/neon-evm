from subprocess import CompletedProcess
import unittest
import random
import re
import string

from solana_utils import *

evm_loader_id = os.environ.get("EVM_LOADER")
'''
USAGE:
    neon-cli [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v, --verbose    Increase message verbosity

OPTIONS:
        --commitment <COMMITMENT_LEVEL>    Return information at the selected commitment level [possible values:
                                           processed, confirmed, finalized] [default: finalized]
    -C, --config <PATH>                    Configuration file to use [default:
                                           /home/neonuser/.config/solana/cli/config.yml]
        --evm_loader <EVM_LOADER>          Pubkey for evm_loader contract
    -u, --url <URL>                        URL for Solana node [default: http://localhost:8899]
    -L, --logging_ctx <LOG_CONTEST>        Logging context

SUBCOMMANDS:
    cancel-trx                Cancel NEON transaction
    create-ether-account      Create ethereum account
    create-program-address    Generate a program address
    deploy                    Deploy a program
    deposit                   Deposit NEONs to ether account
    emulate                   Emulate execution of Ethereum transaction
    get-ether-account-data    Get values stored in associated with given address account data
    get-storage-at            Get Ethereum storage value at given index
    help                      Prints this message or the help of the given subcommand(s)
    migrate-account           Migrates account internal structure to v2
    neon-elf-params           Get NEON values stored in elf
    update-valids-table       Update Valids Table
'''


class NeonCliTest(unittest.TestCase):
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
    #     '''
    #     neon-cli cancel-trx <STORAGE_ACCOUNT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     '''
    #     # storage_account = self.generate_address()
    #     storage_account = self.create_new_account(evm_loader_id)
    #     output = neon_cli().call_v2(
    #         f"cancel-trx {storage_account} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     self.assert_exit_code(output)
    #     # self.print_output(output)

    def test_command_create_ether_account(self):
        '''
        neon-cli create-ether-account <ether> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        ether_account = self.generate_address()
        output = neon_cli().call_v2(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        # expected_line = f""""ether":"{ether_account[2:]}","""
        # self.assertIn(expected_line, output.stdout,
        #               "There is no address in the output")
        self.print_output(output.stdout)

    def test_command_create_program_address(self):
        '''
        neon-cli create-program-address <SEED_STRING> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        # output_re = re.compile(r"^(\w+\s+\d{1,3})$", flags=re.DOTALL)
        seed_string = self.generate_address()
        output = neon_cli().call_v2(
            f"create-program-address {seed_string} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        # self.assertTrue(bool(output_re.search(output.stdout)),
        #                 "The output structure is not 'address nonce'")
        self.print_output(output.stdout)

    '''
    2022-04-01 07:45:08.671 E main.rs:879 300 
    Emulator:Undefined  NeonCli Error (113): Solana client error. 
    ClientError { request: Some(GetMinimumBalanceForRentExemption), 
    kind: RpcError(RpcResponseError { code: -32600, message: "Invalid request", data: Empty }) }
    '''

    @unittest.skip("Invalid request")
    def test_command_deploy(self):
        '''
        neon-cli deploy <PROGRAM_FILEPATH> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        program_filepath = "./neon-cli"
        output = neon_cli().call_v2(
            f"deploy {program_filepath} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    def test_command_emulate(self):
        '''
        neon-cli emulate <SENDER> <CONTRACT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        sender = self.create_new_account(evm_loader_id)
        contract = self.generate_address()
        output = neon_cli().call_v2(
            f"emulate {sender} {contract} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    def test_command_get_ether_account_data(self):
        '''
        neon-cli get-ether-account-data <ether> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        ether_account = self.generate_address()
        neon_cli().call_v2(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        output = neon_cli().call_v2(
            f"get-ether-account-data {ether_account} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)
        self.assert_exit_code(output)

    # def test_command_get_storage_at(self):
    #     '''
    #     neon-cli get-storage-at <contract_id> <index> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     '''
    #     contract_id = self.create_new_account(evm_loader_id)
    #     index = 0
    #     output = neon_cli().call_v2(
    #         f"get-storage-at {contract_id} {index} --evm_loader {evm_loader_id}"
    #     )
    #     self.assertIsNotNone(output)
    #     self.assert_exit_code(output)
    #     # self.print_output(output)

    def test_command_help(self):
        '''
        neon-cli help
        '''
        output = neon_cli().call_without_url(f"help create-ether-account")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        self.print_output(output.stdout)

    # def test_command_migrate_account(self):
    #     '''
    #     neon-cli migrate-account <ETHER> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     '''
    #     ether_account = self.generate_address()
    #     neon_cli().call_v2(
    #         f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
    #     )
    #     output = neon_cli().call_v2(
    #         f"migrate-account {ether_account} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     self.assert_exit_code(output)
    #     # self.print_output(output)

    def test_command_neon_elf_params(self):
        '''
        neon-cli neon-elf-params --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        output_re = re.compile(r"^([\w]+\=\d+)$", flags=re.DOTALL)
        output = neon_cli().call_v2(
            f"neon-elf-params --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        # self.assertTrue(
        #     bool(output_re.search(output.stdout)),
        #     "The output structure is not 'NEON_PARAM=numeric_value'")
        #
        self.print_output(output.stdout)

    # def test_command_update_valids_table(self):
    #     '''
    #     neon-cli update-valids-table <contract_id> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
    #     '''
    #     contract_id = self.generate_address()
    #     output = neon_cli().call_v2(
    #         f"update-valids-table {contract_id} --evm_loader {evm_loader_id}")
    #     self.assertIsNotNone(output)
    #     self.assert_exit_code(output)
    #     # self.print_output(output)

    def test_command_version(self):
        '''
        neon-cli -V
        '''
        output_re = re.compile(r"^neon-cli Neon-cli/v[\d\.]+[\w-]+",
                               flags=re.DOTALL)
        output = neon_cli().call_without_url(f"-V")
        self.assertIsNotNone(output)
        self.assert_exit_code(output)
        # self.assertIn('neon-cli', output.stdout,
        #               "There is no 'neon-cli' in version")
        # self.assertTrue(
        #     bool(output_re.search(output.stdout)),
        #     "The output structure is not 'neon-cli Neon-cli/vNNN-alphanumeric'"
        # )
        self.print_output(output.stdout)

    def generate_address(self) -> str:
        return eth_keys.PrivateKey(os.urandom(32)).public_key.to_address()

    def create_new_account(self, evm_loader_id) -> str:
        ether_account = self.generate_address()
        neon_cli().call_v2(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        return ether_account

    def assert_exit_code(self, result: CompletedProcess):
        self.assertEqual(result.returncode, 0, "Return code is not 0")

    def print_output(self, output: str):
        print("<<<<<<<<<<<<<<<< output >>>>>>>>>>>>>>>>>")
        print(output)


if __name__ == '__main__':
    unittest.main()
