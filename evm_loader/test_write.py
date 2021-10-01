# File: test_write.py
# Test for the Write instruction.
# 1. Checks the operator can write something to a holder account.
# 2. Checks no one other can write to a holder account.

import unittest
from sha3 import shake_256
from solana.publickey import PublicKey
from solana.account import Account as solana_Account
from solana_utils import *

issue = 'https://github.com/neonlabsorg/neon-evm/issues/261'
proxy_id = 1000;
evm_loader_id = os.environ.get('EVM_LOADER')
solana_url = os.environ.get('SOLANA_URL', 'http://localhost:8899')
path_to_solana = 'solana'
client = Client(solana_url)

class Test_Write(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print('\n\n' + issue)
        print('Test_Write')
        cls.init_signer(cls)
        cls.create_account(cls)

    def init_signer(self):
        print('Initializing signer...')
        res = solana_cli().call('config get')
        substr = 'Keypair Path: '
        path = ''
        for line in res.splitlines():
            if line.startswith(substr):
                path = line[len(substr):].strip()
        if path == '':
            raise Exception('cannot get keypair path')
        with open(path.strip(), mode='r') as file:
            pk = (file.read())
            nums = list(map(int, pk.strip('[] \n').split(',')))
            nums = nums[0:32]
            values = bytes(nums)
            self.signer = solana_Account(values)
        print('Signer:', self.signer.public_key())

    def create_account(self):
        print('Creating account...')
        proxy_id_bytes = proxy_id.to_bytes((proxy_id.bit_length() + 7) // 8, 'big')
        signer_public_key_bytes = bytes(self.signer.public_key())
        seed = shake_256(b'holder' + proxy_id_bytes + signer_public_key_bytes).hexdigest(16)
        self.account = accountWithSeed(self.signer.public_key(), seed, PublicKey(evm_loader_id))
        if getBalance(self.account) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.signer.public_key(), self.signer.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            client.send_transaction(trx, self.signer, opts=TxOpts(skip_confirmation=False, preflight_commitment='confirmed'))
        print('Account to write:', self.account)

    # @unittest.skip("a.i.")
    def test_instruction_write_is_ok(self):
        print()

    @unittest.skip("a.i.")
    def test_instruction_write_fails(self):
        print()

    @classmethod
    def tearDownClass(cls):
        pass

if __name__ == '__main__':
    unittest.main()
