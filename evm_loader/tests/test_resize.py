from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
from base58 import b58encode
import web3
from solana_utils import *

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
evm_loader_id = os.environ.get("EVM_LOADER")


class ResizeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_resize.py setUpClass")

        wallet = OperatorAccount(operator1_keypair_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()


    def sol_instr_17_resize(self, address, size) -> Transaction:
        solana_address = PublicKey(self.loader.ether2program(address)[0])
        account_data: bytes = getAccountData(client, solana_address, ACCOUNT_INFO_LAYOUT.sizeof())
        account: AccountInfo = AccountInfo.frombytes(account_data)

        seed = b58encode(ACCOUNT_SEED_VERSION + os.urandom(20)).decode('utf8')
        code_account_new = accountWithSeed(self.acc.public_key(), seed, PublicKey(evm_loader_id))
        minimum_balance = client.get_minimum_balance_for_rent_exemption(size)["result"]

        create_with_seed = createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, minimum_balance, size, PublicKey(evm_loader_id))
        resize = TransactionInstruction(
            program_id=evm_loader_id,
            data=bytearray.fromhex("11") + seed.encode('utf-8'),  # 17- ResizeStorageAccount
            keys=[
                AccountMeta(pubkey=solana_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=account.code_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=code_account_new, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False)
            ]
        )

        trx = TransactionWithComputeBudget()
        trx.add(create_with_seed)
        trx.add(resize)

        return trx



    def test_resize(self):
        account = web3.Account.create()
        self.loader.createEtherAccount(account.address)

        for size in range(1*1024*1024, 10*1024*1024 + 1, 1*1024*1024):
            print("resize account, size = ", size)

            resize_trx = self.sol_instr_17_resize(account.address, size)
            result = send_transaction(client, resize_trx, self.acc)
            print(result)
 

if __name__ == '__main__':
    unittest.main()
