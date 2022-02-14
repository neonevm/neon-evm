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
        balance_re = re.compile(r"^.*balance:\s+(\d+).*$", flags=re.DOTALL)
        # Place deposit to empty account
        neon_cli().call("deposit 10 {} --evm_loader {}".format(ether_account, evm_loader_id))
        # Get account's balance after
        output = neon_cli().call("get-ether-account-data {} --evm_loader {}".format(ether_account, evm_loader_id))
        balance = balance_re.match(output)
        self.assertIsNotNone(balance)
        balance = balance.group(1)
        self.assertEqual(balance, '10000000000')
        # Second deposit (to existing account)
        neon_cli().call("deposit 10 {} --evm_loader {}".format(ether_account, evm_loader_id))
        # Get account's balance after
        output = neon_cli().call("get-ether-account-data {} --evm_loader {}".format(ether_account, evm_loader_id))
        balance = balance_re.match(output)
        self.assertIsNotNone(balance)
        balance = balance.group(1)
        self.assertEqual(balance, '20000000000')

if __name__ == '__main__':
    unittest.main()
