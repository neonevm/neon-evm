import unittest
import re

from solana_utils import *

evm_loader_id = os.environ.get("EVM_LOADER")

class NeonCliTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_neon_cli.py setUpClass")

    def test_command_deposit(self):
        ether_account = eth_keys.PrivateKey(os.urandom(32)).public_key.to_address()
        # Place deposit
        neon_cli().call("deposit 10 {} --evm_loader {}".format(ether_account, evm_loader_id))
        # Get account's balance after
        output = neon_cli().call("get-ether-account-data {} --evm_loader {}".format(ether_account, evm_loader_id))
        balance = re.compile("balance: (.*)")
        balance = p.match(output).group(1)
        self.assertEqual(balance, '10000000000')

if __name__ == '__main__':
    unittest.main()
