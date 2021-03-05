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
http_client = Client(solana_url)
evm_loader = os.environ.get("EVM_LOADER")  #"CLBfz3DZK4VBYAu6pCgDrQkNwLsQphT9tg41h6TQZAh3"
owner_contract = os.environ.get("CONTRACT")  #"HegAG2D9DwRaSiRPb6SaDrmaMYFq9uaqZcn3E1fyYZ2M"
contracts_dir = os.environ.get("CONTRACTS_DIR", "target/bpfel-unknown-unknown/release/")
user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"
#user = "6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2q"


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
        acc = WalletAccount(wallet_path())
        cli = SolanaCli(solana_url, acc)
        result = cli.call('--version')
        print(result)


    def test_solana_deploy(self):
        acc = WalletAccount(wallet_path())
        cli = SolanaCli(solana_url, acc)
        contract = contracts_dir+'../spl_memo.so'
        result = json.loads(cli.call('deploy {}'.format(contract)))
        programId = result['programId']
#        programId = "6H7ruy1fNisqx7zS6DGmb7JTasnBmQVKfk6AMUB58Tui"
        print("Memo program: {}".format(programId))

        def send_memo_trx(data):
            trx = Transaction().add(
                TransactionInstruction(program_id=programId, data=data, keys=[
                    AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                ]))
            res = http_client.send_transaction(trx, self.acc)
            print("Send memo trx:", res)
            print("Done")
            return res["result"]

        trxId = send_memo_trx('hello')
        #confirm_transaction(http_client, trxId)

        err = "Transaction simulation failed: Error processing Instruction 0: invalid instruction data"
        with self.assertRaisesRegex(Exception, err):
            try:
              send_memo_trx(b'\xF0\x9F\x90\xff')
            except Exception as e:
                print("Exception:", e)
                raise




class EvmLoader:
    loader_id = evm_loader

    def __init__(self, solana_url, loader_id=None):
        if not loader_id and not EvmLoader.loader_id:
            print("Load EVM loader...")
            cli = SolanaCli(solana_url)
            contract = contracts_dir+'evm_loader.so'
            result = json.loads(cli.call('deploy {}'.format(contract)))
            programId = result['programId']
            EvmLoader.loader_id = programId
            print("Done\n")

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
        if accs: accounts.extend(accs)

        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader_id, data=data, keys=accounts))
        result = http_client.send_transaction(trx, signer, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
        messages = result["meta"]["logMessages"]
        res = messages[messages.index("Program log: succeed")+1]
        if not res.startswith("Program log: "): raise Exception("Invalid program logs: no result")
        else: return bytearray.fromhex(res[13:])

    def accountWithSeed(self, base, seed, owner):
        seed_data = bytes(seed,'utf8')
        return PublicKey(sha256(bytes(base)+seed_data+bytes(owner)).digest())

    def createAccountWithSeed(self, signer, base, seed, owner, lamports, space):
        seed_data = bytes(seed,'utf8')
        created = self.accountWithSeed(base, seed, owner) #PublicKey(sha256(bytes(base)+seed_data+bytes(owner)).digest())
        print("created: {}".format(created))
        accounts = [
                AccountMeta(pubkey=signer.public_key(), is_signer=False, is_writable=True),
                AccountMeta(pubkey=created, is_signer=False, is_writable=True),
                AccountMeta(pubkey=base, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]

        data =(bytes.fromhex("04000000") +
               bytes(base) +
               bytes.fromhex("%08x"%len(seed_data))[::-1] +
               bytes.fromhex("00000000") +
               seed_data +
               bytes.fromhex("%016x"%lamports)[::-1] +
               bytes.fromhex("%016x"%space)[::-1] +
               bytes(owner)
              )

        print("Data:", data.hex())
        return TransactionInstruction(program_id=self.loader_id, data=data, keys=accounts)

    def createEtherAccount(self, ether):
        cli = SolanaCli(self.solana_url)
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

    def deployChecked(self, location):
        from web3 import Web3
        cli = SolanaCli(self.solana_url)
        account = cli.call("address")
        creator = solana2ether(account)
        with open(location, mode='rb') as file:
            fileHash = Web3.keccak(file.read())
            ether = bytes(Web3.keccak(b'\xff' + creator + bytes(32) + fileHash)[-20:])
        program = self.ether2program(ether)
        info = http_client.get_account_info(program[0])
        print("Creator:", creator.hex(), "program:", program, "info", info)
        if info['result']['value'] is None:
            return self.deploy(location)
        elif info['result']['value']['owner'] != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return {"ethereum": ether.hex(), "programId": program[0]}


def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])


def getBalance(account):
    return http_client.get_balance(account)['result']['value']

@unittest.skip("Need repair")
class EvmLoaderTests2(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.loader = EvmLoader(solana_url)

        # Initialize user account
        cls.acc = Account(b'\xdc~\x1c\xc0\x1a\x97\x80\xc2\xcd\xdfn\xdb\x05.\xf8\x90N\xde\xf5\x042\xe2\xd8\x10xO%/\xe7\x89\xc0<')

        # Create ethereum account for user account
        cls.caller_ether = solana2ether(cls.acc.public_key())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.acc.public_key()) == 0:
            print("Create user account...")
            tx = http_client.request_airdrop(cls.acc.public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
            print("Done\n")

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller_created = cls.loader.createEtherAccount(solana2ether(cls.acc.public_key()))
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller, "({})".format(bytes(PublicKey(cls.caller)).hex()))




    def test_deploy_loader(self):
        loader = EvmLoader(solana_url)



    def test_deploy_owner(self):
        loader = EvmLoader(solana_url)
        ownerId = "ApDWzULkJs7Bcc8VrExMZvVsP2Hbq3tTSs9bGF4AjoKs"
        #ownerId = loader.deploy('owner.bin.3')["programId"]
        print("Owner program:", ownerId)

        result = loader.call(ownerId, caller_program, self.acc, bytearray.fromhex("03893d20e8"))
        print("GetOwner result:", result.hex())

        self.assertEqual(result[0:12], bytes(12))
        self.assertEqual(result[12:], solana2ether("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r"))

        with self.assertRaisesRegex(Exception, "Error processing Instruction 0: invalid instruction data"):
            # Can't change owner because contract was deployed by another account
            result = loader.call(ownerId, caller_program, self.acc, bytearray.fromhex("03a6f9dae1")+bytes(12)+caller_ether)


    def createMint(self, signer):
        return PublicKey('AGoTeNZuy5TVTXcQimVDazFH59TQVcyqJcuPg2yyWyGh')


    def test_deploy_erc20wrapper(self):
        tokenId = PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
        mintId = self.createMint(self.acc)
        mint = Token(http_client, mintId, tokenId, self.acc)
        print("Mint: {} -> 0x{}".format(mintId, bytes(mintId).hex()))

        erc20Id = self.loader.deployChecked("erc20wrapper.bin")["programId"]
        print("ERC20Wrapper program:", erc20Id)
        seed = "btc3"
        seedData = bytes(seed, 'utf8')
        
        # Call setToken for erc20
        data = (bytes.fromhex("036131bdab") + # setToken(uint256,string)
                bytes(mintId) +
                bytes.fromhex("%064x"%0x40) +
                bytes.fromhex("%064x"%len(seedData)) +
                seedData + bytes(32-len(seedData))
               )
        print('setToken arguments:', data.hex())
        result = self.loader.call(
                contract=erc20Id,
                caller=PublicKey(self.caller),
                signer=self.acc,
                data=data,
                accs=None)
        print('setToken result:', result.hex())


        balanceAccount = self.loader.accountWithSeed(PublicKey(self.caller), seed, tokenId)
        balance = http_client.get_balance(balanceAccount)['result']['value']
        if 0 == balance:
            lamports = Token.get_min_balance_rent_for_exempt_for_account(http_client)
            trx = Transaction()
            trx.add(self.loader.createAccountWithSeed(self.acc, PublicKey(self.caller), seed, tokenId, lamports, 165))
            trx.add(TransactionInstruction(program_id=tokenId, data=bytes.fromhex('01'), keys=[
                    AccountMeta(pubkey=balanceAccount, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=mintId, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=PublicKey(self.caller), is_signer=False, is_writable=False),
                    AccountMeta(pubkey=SYSVAR_RENT_PUBKEY, is_signer=False, is_writable=False),
                ]))

            result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
            print("createAccountWithSeed:", result)

        print("Balance {} {}: {}".format(
                balanceAccount, bytes(balanceAccount).hex(),
                mint.get_balance(balanceAccount)['result']['value']['uiAmount']))


        # Call transfer(uint256, uint256)
        to = PublicKey("EDPtG7cJ5eEBREiTU6QyGktK4kBXwFcurGKERcZaXgJo")
        data =(bytes.fromhex("030cf79e0a") +
               bytes(to) + bytes.fromhex('%064x'%(1000000000))
              )
        result = self.loader.call(
                contract=erc20Id,
                caller=PublicKey(self.caller),
                signer=self.acc,
                data=data,
                accs=[
                    AccountMeta(pubkey=tokenId, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=balanceAccount, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=to, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=mintId, is_signer=False, is_writable=False),
                ]
            )
        print('transfer result:', result.hex())



    def test_deployChecked(self):
        loader = EvmLoader(solana_url)
        loader.deployChecked("erc20wrapper.bin")



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




@unittest.skip("Need repair")
class EvmLoaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
#        cls.acc = Account(b'\xdc~\x1c\xc0\x1a\x97\x80\xc2\xcd\xdfn\xdb\x05.\xf8\x90N\xde\xf5\x042\xe2\xd8\x10xO%/\xe7\x89\xc0<')
#        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
#        print('Private:', cls.acc.secret_key())
#        balance = http_client.get_balance(cls.acc.public_key())['result']['value']
#        if balance == 0:
#            tx = http_client.request_airdrop(cls.acc.public_key(), 10*10**9)
#            confirm_transaction(http_client, tx['result'])
#            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
#        print('Balance:', balance)
#
#        # caller created with "50b41b481f04ac2949c9cc372b8f502aa35bddd1" ethereum address
#        cls.caller = PublicKey("A8semLLUsg5ZbhACjD2Vdvn8gpDZV1Z2dPwoid9YUr4S")
#
        cls.loader = EvmLoader(solana_url)

        # Initialize user account
        cls.acc = Account(b'\xdc~\x1c\xc0\x1a\x97\x80\xc2\xcd\xdfn\xdb\x05.\xf8\x90N\xde\xf5\x042\xe2\xd8\x10xO%/\xe7\x89\xc0<')

        # Create ethereum account for user account
        cls.caller_ether = solana2ether(cls.acc.public_key())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.acc.public_key()) == 0:
            print("Create user account...")
            tx = http_client.request_airdrop(cls.acc.public_key(), 10*10**9)
            confirm_transaction(http_client, tx['result'])
            balance = http_client.get_balance(cls.acc.public_key())['result']['value']
            print("Done\n")

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller_created = cls.loader.createEtherAccount(solana2ether(cls.acc.public_key()))
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller, "({})".format(bytes(PublicKey(cls.caller)).hex()))

        cls.contract = cls.loader.deployChecked(contracts_dir+"Owner.binary")["programId"]
        print("Contract:", cls.contract)

    def test_call_getOwner(self):
        data = bytearray.fromhex("03893d20e8")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=self.contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)

    def test_call_changeOwner(self):
        data = bytearray.fromhex("03a6f9dae10000000000000000000000005b38da6a701c568545dcfcb03fcb875f56beddc4")
        trx = Transaction().add(
            TransactionInstruction(program_id=evm_loader, data=data, keys=[
                AccountMeta(pubkey=self.contract, is_signer=False, is_writable=True),
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
                AccountMeta(pubkey=self.contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ]))
        result = http_client.send_transaction(trx, self.acc)


if __name__ == '__main__':
    unittest.main()

