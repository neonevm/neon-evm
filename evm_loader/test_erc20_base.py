from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
import time

http_client = Client("http://localhost:8899")
evm_loader = "QmPLKib82RHRE3McFECd4xWJyJtahaHpCPzAPz5JNMp"
erc20 = "CY6xpQ4pMtpCf92KU3pnVieNNKo6dHcv2PKBdNcLUMEk"

#  0000000000000000000000000000000000001111  - ether addr
caller = "DrJQU8ZDVNaz46vkwUkYVri2yPXkSZc9nn2jvpS84Xf5"


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
        cls.acc = Account(
            [209, 145, 218, 165, 152, 167, 119, 103, 234, 226, 29, 51, 200, 101, 66, 47, 149, 160, 31, 112, 91, 196,
             251, 239, 130, 113, 212, 97, 119, 176, 117, 190])
        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print('Private:', cls.acc.secret_key())
        balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        if balance == 0:
            tx = http_client.request_airdrop(cls.acc.public_key(), 10 * 10 ** 9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        print('Balance:', balance)


    def test_deposit(self):
        input = bytearray.fromhex(
            "0300aeef8afb3f13832afb4a4d6a0331c6d5c840506efd28bb1c64889e1c57145bfaa582a29428bde187afe4a2a8f44c662f122f667ee36353209fec99bfb72be01324f2ca0000000000000000000000000000000000000000000000000000000000000001"
            )
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=input, keys=
            [
                AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
                # from
                AccountMeta(pubkey="Hum7npB6PTTNyn26Pnt7XtHHRaujhVxDPRh5tuf2nZ8u", is_signer=False, is_writable=True),
                # to
                AccountMeta(pubkey="5kag8i1weEyDKA9MeL1iM717zLk3zDBCzrsZyfN5HnXL", is_signer=False, is_writable=True),
                # mint_id
                AccountMeta(pubkey="3U8PaKXsQjcZJPHE8nThgjpK3hbYmGzJ5iDHQJKNYVxQ", is_signer=False, is_writable=False),
                AccountMeta(pubkey="TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", is_signer=False, is_writable=False),
                # signer
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)
        result = confirm_transaction(http_client, result["result"])
        print(result["result"])

if __name__ == '__main__':
    unittest.main()
