import unittest
import solana
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana_utils import *


class FundReturnTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_fund_return.py setUpClass")

        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(wallet, EVM_LOADER)
        cls.acc = wallet.get_acc()

        cls.alice_acc = cls.create_account(cls, "alice")
        cls.bob_acc = cls.create_account(cls, "bob")

        cls.refundable_seed = "refund"
        cls.refundable_acc = cls.create_account_with_seed_from_acc(cls, cls.alice_acc, cls.refundable_seed)

        cls.fail_seed = "fail"
        cls.fail_acc = cls.create_account_with_seed_from_acc(cls, cls.alice_acc, cls.fail_seed)


    def create_account(self, seed):
        bytes_seed = sha256(bytes(seed, 'utf8'))
        new_acc = Account(bytes_seed)
        trx = client.request_airdrop(new_acc.public_key(), 10 * 10 ** 9)
        confirm_transaction(client, trx['result'])
        return new_acc


    def create_account_with_seed_from_acc(self, acc, seed):
        storage = PublicKey(sha256(bytes(acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10**9, 128*1024, PublicKey(EVM_LOADER)))
            send_transaction(client, trx, acc)

        return storage


    def call_refund_tx(self, del_key, acc, seed, signer):
        trx = Transaction()
        trx.add(TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("10") + bytes(seed, 'utf8'),
            keys=[
                AccountMeta(pubkey=del_key, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=(signer==acc), is_writable=True),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
            ]))
        return send_transaction(client, trx, signer)


    def test_creator_not_signer(self):
        err_msg = "Creator acc must be signer."

        try:
            self.call_refund_tx(self.fail_acc, self.alice_acc, self.fail_seed, self.bob_acc)
        except solana.rpc.api.SendTransactionError as err:
            self.assertTrue(err_msg in str(err.result))
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            self.assertTrue(False)
        else:
            self.assertTrue(False)


    def test_error_on_wrong_creator(self):
        err_msg = "Deleted account info doesn't equal to generated."

        try:
            self.call_refund_tx(self.fail_acc, self.bob_acc, self.fail_seed, self.bob_acc)
        except solana.rpc.api.SendTransactionError as err:
            self.assertTrue(err_msg in str(err.result))
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            self.assertTrue(False)
        else:
            self.assertTrue(False)


    def test_error_on_wrong_seed(self):
        err_msg = "Deleted account info doesn't equal to generated."

        try:
            self.call_refund_tx(self.fail_acc, self.alice_acc, self.refundable_seed, self.alice_acc)
        except solana.rpc.api.SendTransactionError as err:
            self.assertTrue(err_msg in str(err.result))
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            self.assertTrue(False)
        else:
            self.assertTrue(False)


    def test_success_refund(self):
        result = self.call_refund_tx(self.refundable_acc, self.alice_acc, self.refundable_seed, self.alice_acc)
        print(result)


if __name__ == '__main__':
    unittest.main()
