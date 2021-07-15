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
from spl.token.instructions import get_associated_token_address

import subprocess

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
client = Client(solana_url)


class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_remove_caller.py setUpClass")

        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        program_and_code = cls.loader.deployChecked(
                CONTRACTS_DIR+'helloWorld.binary',
                cls.caller, cls.caller_ether
            )
        cls.owner_contract = program_and_code[0]
        cls.contract_code = program_and_code[2]
        
        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())
        print("code id: ", cls.contract_code)

    def test_call_by_some_caller(self):
        call_hello = bytearray.fromhex("033917b3df")
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=call_hello, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.owner_contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=get_associated_token_address(self.acc.public_key(), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = send_transaction(client, trx, self.acc)
        print(result)

    def test_call_by_signer(self):
        # Check that another account can't use Ethereum address
        acc = Account()
        print("acc:", acc.public_key())
        if getBalance(acc.public_key()) == 0:
            print("request_airdrop for ", acc.public_key())
            tx = client.request_airdrop(acc.public_key(), 10*10**9)
            confirm_transaction(client, tx['result'])
            balance = client.get_balance(acc.public_key())['result']['value']
            print("Done\n")
        call_hello = bytearray.fromhex("033917b3df")
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=call_hello, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.owner_contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=get_associated_token_address(acc.public_key(), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))

        #err = "invalid program argument"
        err = "Transaction simulation failed: Error processing Instruction 0: invalid program argument"
        with self.assertRaisesRegex(Exception,err):
            result = send_transaction(client, trx, acc)
            print(result)


if __name__ == '__main__':
    unittest.main()
