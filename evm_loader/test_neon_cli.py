import unittest
import os

evm_loader_id = os.environ.get("EVM_LOADER")

ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))

class NeonCliTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_neon_cli.py setUpClass")

    def test_command_deposit(self):
        empty_account: bytes = eth_keys.PrivateKey(os.urandom(32)).public_key.to_canonical_address()
        neon_cli().call("deposit 1 {} --evm_loader {}".format(empty_account, evm_loader_id))

if __name__ == '__main__':
    unittest.main()
