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

    def test_command_create_ether_account(self):
        '''
        neon-cli create-ether-account <ether> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        ether_account = eth_keys.PrivateKey(
            os.urandom(32)).public_key.to_address()
        output = neon_cli().call(
            f"create-ether-account {ether_account} --evm_loader {evm_loader_id}"
        )
        self.assertIsNotNone(output)

    def test_command_create_program_address(self):
        '''
        neon-cli create-program-address <SEED_STRING> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        seed_string = ''.join([
            ''.join([random.choice(string.ascii_lowercase)
                     for y in range(5)]) + ' ' for x in range(24)
        ]).strip()
        output = neon_cli().call(
            f"create-ether-account {seed_string} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)

    def test_command_emulate(self):
        '''
        neon-cli emulate <SENDER> <CONTRACT> --commitment <COMMITMENT_LEVEL> --config <PATH> --url <URL>
        '''
        sender = eth_keys.PrivateKey(
            os.urandom(32)).public_key.to_address()
        contract = ""
        output = neon_cli().call(
            f"emulate {sender} {contract} --evm_loader {evm_loader_id}")
        self.assertIsNotNone(output)



if __name__ == '__main__':
    unittest.main()
