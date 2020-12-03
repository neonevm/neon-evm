from solana.rpc.api import Client
from solana.account import Account
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
import time

http_client = Client("http://localhost:8899")
evm_loader = "7Cdv9VeQEbBmXvJZaE4vmGkyobr1zUAJ8yu8urKFxpBM"
owner_contract = "GoXMtBXLRLU3mQsfKtvTD2bWqmkfvSZmhcaTdCyC6vVk"
user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"



def confirm_transaction(client, tx_sig):
    """Confirm a transaction."""
    TIMEOUT = 30  # 30 seconds  pylint: disable=invalid-name
    elapsed_time = 0
    while elapsed_time < TIMEOUT:
        sleep_time = 3
        if not elapsed_time:
            sleep_time = 7
            time.sleep(sleep_time)
        else:
            time.sleep(sleep_time)
        resp = client.get_confirmed_transaction(tx_sig)
        if resp["result"]:
#            print('Confirmed transaction:', resp)
            break
        elapsed_time += sleep_time
    if not resp["result"]:
        raise RuntimeError("could not confirm transaction: ", tx_sig)
    return resp





class EvmLoaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.acc = Account(b'\xdc~\x1c\xc0\x1a\x97\x80\xc2\xcd\xdfn\xdb\x05.\xf8\x90N\xde\xf5\x042\xe2\xd8\x10xO%/\xe7\x89\xc0<')
        print('Account:', cls.acc.public_key())
        print('Private:', cls.acc.secret_key())
        balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        if balance == 0:
            tx = http_client.request_airdrop(cls.acc.public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        print('Balance:', balance)


    def test_call_getOwner(self):
        data = bytearray.fromhex("b7f05836")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)


if __name__ == '__main__':
    unittest.main()

