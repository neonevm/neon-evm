from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
import base58
from eth_account import Account as EthAccount

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")

class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.acc = WalletAccount(wallet_path())

        # print(bytes(cls.acc.public_key()).hex())
        if getBalance(cls.acc.get_acc().public_key()) == 0:
            print("request_airdrop for ", cls.acc.get_acc().public_key())
            cli = SolanaCli(solana_url, cls.acc)
            cli.call('airdrop 1000000')
            # tx = http_client.request_airdrop(cls.acc.get_acc().public_key(), 100000)
            # confirm_transaction(http_client, tx['result'])
            # balance = http_client.get_balance(cls.acc.get_acc().public_key())['result']['value']
            print("Done\n")
            
        cls.loader = EvmLoader(solana_url, cls.acc)
        # cls.loader = EvmLoader(solana_url, cls.acc, 'ChcwPA3VHaKHEuzikJXHEy6jP5Ycn9ZV7KYZXfeiNp5m')
        cls.evm_loader = cls.loader.loader_id
        print("evm loader id: ", cls.evm_loader)
        cls.owner_contract = cls.loader.deployChecked(
                CONTRACTS_DIR+'helloWorld.binary',
                solana2ether(cls.acc.get_acc().public_key())
            )[0]
        # cls.owner_contract = "HAAfFJK4tsJb38LC2MULMzgpYkqAKRguyq7GRTocvGE9"
        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())

        cls.eth_caller = EthAccount.from_key(cls.acc.get_acc().secret_key())
        cls.sol_caller = cls.loader.ether2program(cls.eth_caller.address)[0]
        print("Caller:", cls.eth_caller.address, cls.sol_caller)
        if getBalance(cls.sol_caller) == 0:
            cls.loader.createEtherAccount(cls.eth_caller.address)

    def test_success_tx_send(self):  
        tx_1 = {
            'to': solana2ether(self.owner_contract),
            'value': 1,
            'gas': 1,
            'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.sol_caller),
            'data': '3917b3df',
            'chainId': 1
        }
        
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())
        keccak_instruction = make_keccak_instruction_data(1, len(msg))

        trx_data = bytearray.fromhex(self.eth_caller.address[2:]) + sign + msg
        
        trx = Transaction().add(
            TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
                AccountMeta(pubkey=self.sol_caller, is_signer=False, is_writable=False),
            ])).add(
            TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("05") + trx_data, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.sol_caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
                AccountMeta(pubkey=self.evm_loader, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
            ]))
        result = http_client.send_transaction(trx, self.acc.get_acc(), opts=TxOpts(skip_confirmation=False))

    # def test_fail_on_no_signature(self):  
    #     tx_1 = {
    #         'to': solana2ether(self.owner_contract),
    #         'value': 1,
    #         'gas': 1,
    #         'gasPrice': 1,
    #         'nonce': 0,
    #         'data': '3917b3df',
    #         'chainId': 1
    #     }
        
    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())
    #     keccak_instruction = make_keccak_instruction_data(1, len(msg))
        
    #     (caller, caller_nonce) = self.loader.ether2program(from_addr)
    #     print(" ether: " + from_addr.hex())
    #     print("solana: " + caller)
    #     print(" nonce: " + str(caller_nonce))

    #     if getBalance(caller) == 0:
    #         print("Create caller account...")
    #         caller_created = self.loader.createEtherAccount(from_addr)
    #         print("Done\n")

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("a1") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
    #             AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
    #         ]))
    #     result = http_client.send_transaction(trx, self.acc.get_acc())


    # def test_check_wo_checks(self):  
    #     tx_1 = {
    #         'to': solana2ether(self.owner_contract),
    #         'value': 0,
    #         'gas': 0,
    #         'gasPrice': 0,
    #         'nonce': 0,
    #         'data': '3917b3df',
    #         'chainId': 1
    #     }
        
    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())

    #     keccak_instruction = make_keccak_instruction_data(1, len(msg))

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
    #             AccountMeta(pubkey=PublicKey("KeccakSecp256k11111111111111111111111111111"), is_signer=False, is_writable=False),
    #         ])).add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("05") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
    #             AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
    #         ]))
    #     result = http_client.send_transaction(trx, self.acc.get_acc())

    # def test_raw_tx_wo_checks(self):  
    #     tx_2 = "0xf86180808094535d33341d2ddcc6411701b1cf7634535f1e8d1680843917b3df26a013a4d8875dfc46a489c2641af798ec566d57852b94743b234517b73e239a5a22a07586d01a8a1125be7108ee6580c225a622c9baa0938f4d08abe78556c8674d58"
        
    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_2)

    #     keccak_instruction = make_keccak_instruction_data(1, len(msg))

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
    #             AccountMeta(pubkey=PublicKey("KeccakSecp256k11111111111111111111111111111"), is_signer=False, is_writable=False),
    #         ])).add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("05") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
    #             AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
    #         ]))
    #     result = http_client.send_transaction(trx, self.acc.get_acc())


if __name__ == '__main__':
    unittest.main()
