import unittest

from solana_utils import *

evm_loader_id = os.environ.get("EVM_LOADER")

class NeonCliTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_neon_cli.py setUpClass")

    def test_command_deposit(self):
        ether_account = eth_keys.PrivateKey(os.urandom(32)).public_key.to_address()
        neon_cli().call("deposit 10 {} --evm_loader {}".format(ether_account, evm_loader_id))
        output = neon_cli().call("get-ether-account-data {} --evm_loader {}".format(ether_account, evm_loader_id))
        print(output)

if __name__ == '__main__':
    unittest.main()
