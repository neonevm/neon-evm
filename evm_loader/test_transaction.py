from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
import base58

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from solana_utils import *

class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # cls.acc = RandomAccaunt()
        cls.acc = RandomAccaunt('1613586854.json')
        if getBalance(cls.acc.get_acc().public_key()) == 0:
            print("request_airdrop for ", cls.acc.get_acc().public_key())
            cli = SolanaCli(solana_url, cls.acc)
            cli.call('airdrop 1000000')
            # tx = http_client.request_airdrop(cls.acc.get_acc().public_key(), 100000)
            # confirm_transaction(http_client, tx['result'])
            # balance = http_client.get_balance(cls.acc.get_acc().public_key())['result']['value']
            print("Done\n")
            
        # cls.loader = EvmLoader(solana_url, cls.acc)
        cls.loader = EvmLoader(solana_url, cls.acc, 'BSU1sPKutxTeeRN8zqhvESXCAE4Xbap3CmsLuMfpLho2')
        cls.evm_loader = cls.loader.loader_id
        print("evm loader id: ", cls.evm_loader)
        # cls.owner_contract = cls.loader.deploy('evm_loader/hello_world.bin')
        cls.owner_contract = "a8CsHohzxZb67uEJHsocq2NRqnSVQ9Xxqiedu9hFDkE"
        print("contract id: ", cls.owner_contract)
        print("contract id: ", solana2ether(cls.owner_contract).hex())

        cls.caller_ether = solana2ether(cls.acc.get_acc().public_key())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller_created = cls.loader.createEtherAccount(solana2ether(cls.acc.get_acc().public_key()))
            print("Done\n")

        print('Account:', cls.acc.get_acc().public_key(), bytes(cls.acc.get_acc().public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller, "({})".format(bytes(PublicKey(cls.caller)).hex()))

    def test_check_tx(self):  
        tx_1 = {
            'to': solana2ether(self.owner_contract),
            'value': 1,
            'gas': 1,
            'gasPrice': 1,
            'nonce': 0,
            'data': '3917b3df',
            'chainId': 1
        } 
        
        (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())
        print(from_addr.hex())
        print(self.caller_ether.hex())
        print(base58.b58decode(self.acc.get_acc().public_key().to_base58()).hex())

        keccak_instruction = make_keccak_instruction_data(0, len(msg))

        trx = Transaction().add(
            TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction + from_addr + sign + msg, keys=[
                AccountMeta(pubkey=PublicKey("KeccakSecp256k11111111111111111111111111111"), is_signer=False, is_writable=False),
            ])).add(
            TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("a1"), keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
            ]))
        result = http_client.send_transaction(trx, self.acc.get_acc())


if __name__ == '__main__':
    unittest.main()
