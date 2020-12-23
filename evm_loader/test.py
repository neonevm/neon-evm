from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
import time
import os
import json

import subprocess

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
evm_loader = os.environ.get("EVM_LOADER")  #"CLBfz3DZK4VBYAu6pCgDrQkNwLsQphT9tg41h6TQZAh3"
owner_contract = os.environ.get("CONTRACT")  #"HegAG2D9DwRaSiRPb6SaDrmaMYFq9uaqZcn3E1fyYZ2M"
user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"
#user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2q"

if evm_loader is None:
    print("Please set EVM_LOADER environment")
    exit(1)

#if owner_contract is None:
#    print("Please set CONTRACT environment")
#    exit(1)

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



class SolanaCli:
    def __init__(self, url):
        self.url = url

    def call(self, arguments):
        cmd = 'solana --url {} {}'.format(self.url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise


class SolanaCliTests(unittest.TestCase):
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
       

    def test_solana_cli(self):
        cli = SolanaCli(solana_url)
        result = cli.call('--version')
        print(result)


    def test_solana_deploy(self):
        cli = SolanaCli(solana_url)
        contract = 'target/bpfel-unknown-unknown/release/spl_memo.so'
        result = json.loads(cli.call('deploy {}'.format(contract)))
        programId = result['programId']
#        programId = "6H7ruy1fNisqx7zS6DGmb7JTasnBmQVKfk6AMUB58Tui"
        print("Memo program: {}".format(programId))

        def send_memo_trx(data):
            trx = Transaction().add(
                TransactionInstruction(program_id=programId, data=data, keys=[
                    AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                ]))
            return http_client.send_transaction(trx, self.acc)["result"]

        trxId = send_memo_trx('hello')
        #confirm_transaction(http_client, trxId)

        err = "Transaction simulation failed: Error processing Instruction 0: invalid instruction data"
        with self.assertRaisesRegex(Exception, err):
            send_memo_trx(b'\xF0\x9F\x90\xff')




class EvmLoader:
    loader_id = None or 'G4U8QxK9DnvjxHE5rxMgt8tsjkBK1zevsi4mAsfTxfPA'

    def __init__(self, solana_url, loader_id=None):
        if not loader_id and not EvmLoader.loader_id:
            cli = SolanaCli(solana_url)
            contract = 'target/bpfel-unknown-unknown/release/evm_loader.so'
            result = json.loads(cli.call('deploy {}'.format(contract)))
            programId = result['programId']
            EvmLoader.loader_id = programId

        self.solana_url = solana_url
        self.loader_id = loader_id or EvmLoader.loader_id
        print("Evm loader program: {}".format(self.loader_id))

    def deploy(self, contract):
        cli = SolanaCli(self.solana_url)
        output = cli.call("deploy --use-evm-loader {} {}".format(self.loader_id, contract))
        print(type(output), output)
        return json.loads(output.splitlines()[-1])

    def call(self, contract, caller, signer, data, accs=None):
        accounts = [
                AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=signer.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]
        if accs: accounts.append(accs)

        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader_id, data=data, keys=accounts))
        result = http_client.send_transaction(trx, signer, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
        messages = result["meta"]["logMessages"]
        res = messages[messages.index("Program log: succeed")+1]
        if not res.startswith("Program log: "): raise Exception("Invalid program logs: no result")
        else: return bytearray.fromhex(res[13:])

    def createEtherAccount(self, ether):
        cli = SolanaCli(self.solana_url)
        try:
            (prog,nonce) = self.ether2program(ether)
            cli.call("account {}".format(prog))
            return prog
        except:
            output = cli.call("create-ether-account {} {} 1".format(self.loader_id, ether.hex()))
            result = json.loads(output.splitlines()[-1])
            return result["solana"]

    def ether2program(self, ether):
        cli = SolanaCli(self.solana_url)
        output = cli.call("create-program-address {} {}".format(ether.hex(), self.loader_id))
        items = output.rstrip().split('  ')
        return (items[0], int(items[1]))

    def checkAccount(self, solana):
        info = http_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))


def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])


class EvmLoaderTests2(unittest.TestCase):
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
        evm_loader = EvmLoader(solana_url)
        cls.caller = evm_loader.createEtherAccount(solana2ether(cls.acc.public_key()))



    def test_deploy_evm_loader(self):
        evm_loader = EvmLoader(solana_url)



    def test_deploy_owner(self):
        loader = EvmLoader(solana_url)
        ownerId = "ApDWzULkJs7Bcc8VrExMZvVsP2Hbq3tTSs9bGF4AjoKs"
        #ownerId = loader.deploy('owner.bin.3')["programId"]
        print("Owner program:", ownerId)

        caller_ether = solana2ether(self.acc.public_key())
        (caller_program, caller_nonce) = loader.ether2program(caller_ether)
        print("Caller:", caller_ether.hex())
        result = loader.call(ownerId, caller_program, self.acc, bytearray.fromhex("03893d20e8"))
        print("GetOwner result:", result.hex())

        self.assertEqual(result[0:12], bytes(12))
        self.assertEqual(result[12:], solana2ether("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"))

        with self.assertRaisesRegex(Exception, "Error processing Instruction 0: invalid instruction data"):
            # Can't change owner because contract was deployed by another account
            result = loader.call(ownerId, caller_program, self.acc, bytearray.fromhex("03a6f9dae1")+bytes(12)+caller_ether)



    def test_deploy_erc20wrapper(self):
        loader = EvmLoader(solana_url)
        ownerId = ""
        ownerId = loader.deploy('



    def test_address_conversions(self):
        ''' This tests check address convertions:
            - Solana -> Ethereum
            - Ethereum -> Solana program_address
            (Python implementation create_program_address not worked yet, so we use solana cli)
        '''
        loader = EvmLoader(solana_url, "AXn5Wa1iZPkkjeRmhPwh3uZidt6nLmwE4cbjYXYue9wL")
        ether = solana2ether("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r")
        self.assertEqual(ether.hex(), "6150976660fd363fbbf2c6ce87da0002c24c0d81")

        (solana, nonce) = loader.ether2program(ether)
        self.assertEqual(solana, "EDy1dxh381pTJYytTwawYbcDT4UanYRiyAk6NuixPcdV")
        self.assertEqual(nonce, 253)



    def test_check_account(self):
        evm_loader = EvmLoader(solana_url)
        evm_loader.checkAccount("ApDWzULkJs7Bcc8VrExMZvVsP2Hbq3tTSs9bGF4AjoKs")
        evm_loader.checkAccount("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r")




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

