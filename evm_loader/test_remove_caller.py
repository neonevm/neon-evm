from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.sysvar import *
from solana.rpc.types import TxOpts
import unittest
import time
import os
import json
from hashlib import sha256
from spl.token.client import Token
from solana_utils import *

import subprocess

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
http_client = Client(solana_url)


class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.acc = WalletAccount(wallet_path())
        if getBalance(cls.acc.get_acc().public_key()) == 0:
            print("request_airdrop for ", cls.acc.get_acc().public_key())
            tx = http_client.request_airdrop(cls.acc.get_acc().public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.get_acc().public_key())['result']['value']
            print("Done\n")
            
        cls.loader = EvmLoader( cls.acc)
        cls.evm_loader = cls.loader.loader_id
        print("evm loader id: ", cls.evm_loader)
        program_and_code = cls.loader.deployChecked(
                CONTRACTS_DIR+'helloWorld.binary',
                solana2ether(cls.acc.get_acc().public_key())
            )
        cls.owner_contract = program_and_code[0]
        cls.contract_code = program_and_code[2]
        
        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())
        print("code id: ", cls.contract_code)

        cls.caller_ether = solana2ether(cls.acc.get_acc().public_key())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller_created = cls.loader.createEtherAccount(solana2ether(cls.acc.get_acc().public_key()))
            print("Done\n")

        print('Account:', cls.acc.get_acc().public_key(), bytes(cls.acc.get_acc().public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller, "({})".format(bytes(PublicKey(cls.caller)).hex()))

    def test_call_by_some_caller(self):
        call_hello = bytearray.fromhex("033917b3df")
        trx = Transaction().add(
            TransactionInstruction(program_id=self.evm_loader, data=call_hello, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.evm_loader, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc.get_acc())
        print(result)

    def test_call_by_self(self):
        call_hello = bytearray.fromhex("033917b3df")
        trx = Transaction().add(
            TransactionInstruction(program_id=self.evm_loader, data=call_hello, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.evm_loader, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc.get_acc())
        print(result)

    def test_call_by_signer(self):
        # Check that another account can't use Ethereum address
        acc = Account()
        print("acc:", acc.public_key())
        if getBalance(acc.public_key()) == 0:
            print("request_airdrop for ", acc.public_key())
            tx = http_client.request_airdrop(acc.public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(acc.public_key())['result']['value']
            print("Done\n")
        call_hello = bytearray.fromhex("033917b3df")
        trx = Transaction().add(
            TransactionInstruction(program_id=self.evm_loader, data=call_hello, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.evm_loader, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))

        #err = "invalid program argument"
        err = "Failed to send transaction"
        with self.assertRaisesRegex(Exception,err):
            result = http_client.send_transaction(trx, acc)
            print(result)


if __name__ == '__main__':
    unittest.main()
