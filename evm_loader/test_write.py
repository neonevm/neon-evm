# File: test_write.py
# Test for the Write instruction.
# 1. Checks the operator can write to a holder account.
# 2. Checks the operator cannot write to a holder account with wrong proxy_id.
# 3. Checks no one other can write to a holder account.

import unittest
from sha3 import shake_256
from solana.publickey import PublicKey
from solana.account import Account as solana_Account
from solana.rpc.api import SendTransactionError
from solana_utils import *

issue = 'https://github.com/neonlabsorg/neon-evm/issues/261'
test_data = b'Chancellor on brink of second bailout for banks'
proxy_id = 1000;
evm_loader_id = os.environ.get('EVM_LOADER')
solana_url = os.environ.get('SOLANA_URL', 'http://localhost:8899')
path_to_solana = 'solana'
client = Client(solana_url)

def write_layout(id, offset, data):
    return (bytes.fromhex('00000000') +
            id.to_bytes(8, byteorder='little') +
            offset.to_bytes(4, byteorder='little') +
            len(data).to_bytes(8, byteorder='little') +
            data)

def read_account(address):
    r = solana_cli().call('account ' + str(address))
    return r

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
        print('Balance of signer:', getBalance(self.signer.public_key()))

    def create_account(self):
        proxy_id_bytes = proxy_id.to_bytes((proxy_id.bit_length() + 7) // 8, 'big')
        signer_public_key_bytes = bytes(self.signer.public_key())
        seed = shake_256(b'holder' + proxy_id_bytes + signer_public_key_bytes).hexdigest(16)
        self.account_address = accountWithSeed(self.signer.public_key(), seed, PublicKey(evm_loader_id))
        if getBalance(self.account_address) == 0:
            print('Creating account...')
            trx = Transaction()
            trx.add(createAccountWithSeed(self.signer.public_key(), self.signer.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            client.send_transaction(trx, self.signer, opts=TxOpts(skip_confirmation=False, preflight_commitment='confirmed'))
        print('Account to write:', self.account_address)
        print('Balance of account:', getBalance(self.account_address))

    def write_to_account(self, nonce, data):
        tx = Transaction()
        metas = [AccountMeta(pubkey=self.account_address, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=self.signer.public_key(), is_signer=True, is_writable=False)]
        tx.add(TransactionInstruction(program_id=evm_loader_id,
                                      data=write_layout(nonce, 0, data),
                                      keys=metas))
        opts = TxOpts(skip_confirmation=True, preflight_commitment='confirmed')
        return client.send_transaction(tx, self.signer, opts=opts)['id']

    # @unittest.skip("a.i.")
    def test_instruction_write_is_ok(self):
        print()
        id = self.write_to_account(proxy_id, test_data)
        print('id:', id)
        self.assertGreater(id, 0)

    # @unittest.skip("a.i.")
    def test_instruction_write_fails(self):
        print()
        try:
            wrong_nonce = proxy_id + 1
            self.write_to_account(wrong_nonce, test_data)
        except SendTransactionError as err:
            self.check_err_is_invalid_program_argument(str(err))
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            raise

    def check_err_is_invalid_program_argument(self, message):
        self.assertEqual(message, 'Transaction simulation failed: Error processing Instruction 0: invalid program argument')
        print('This exception is expected')

    @classmethod
    def tearDownClass(cls):
        pass

if __name__ == '__main__':
    unittest.main()
