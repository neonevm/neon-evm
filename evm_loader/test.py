from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
import time
import os

http_client = Client("http://localhost:8899")
evm_loader = os.environ.get("EVM_LOADER")  #"CLBfz3DZK4VBYAu6pCgDrQkNwLsQphT9tg41h6TQZAh3"
owner_contract = os.environ.get("CONTRACT")  #"HegAG2D9DwRaSiRPb6SaDrmaMYFq9uaqZcn3E1fyYZ2M"
user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"
#user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2q"

if evm_loader is None:
    print("Please set EVM_LOADER environment")
    exit(1)

if owner_contract is None:
    print("Please set CONTRACT environment")
    exit(1)

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
        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print('Private:', cls.acc.secret_key())
        balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        if balance == 0:
            tx = http_client.request_airdrop(cls.acc.public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
        print('Balance:', balance)

        # caller created with "50b41b481f04ac2949c9cc372b8f502aa35bddd1" ethereum address
        cls.caller = PublicKey("A8semLLUsg5ZbhACjD2Vdvn8gpDZV1Z2dPwoid9YUr4S")


    def test_call_getOwner(self):
        data = bytearray.fromhex("03893d20e8")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)

    def test_call_changeOwner(self):
        data = bytearray.fromhex("03a6f9dae10000000000000000000000005b38da6a701c568545dcfcb03fcb875f56beddc4")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey="6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r", is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)


    def test_call(self):
        data = bytearray.fromhex("03893d20e8")
        #data = (1024*1024-1024).to_bytes(4, "little")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)


if __name__ == '__main__':
    unittest.main()

