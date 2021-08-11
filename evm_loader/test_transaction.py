from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
import base58
from eth_account import Account as EthAccount
from spl.token.instructions import get_associated_token_address

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
evm_loader_id = os.environ.get("EVM_LOADER")

class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_transaction.py setUpClass")

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
                cls.caller,
                cls.caller_ether
            )
        cls.owner_contract = program_and_code[0]
        cls.contract_code = program_and_code[2]
        
        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())
        print("code id: ", cls.contract_code)


    def test_success_tx_send(self):  
        tx_1 = {
            'to': solana2ether(self.owner_contract),
            'value': 0,
            'gas': 9999999,
            'gasPrice': 1,
            'nonce': getTransactionCount(client, self.caller),
            'data': '3917b3df',
            'chainId': 111
        }
        
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx_1, self.acc.secret_key())
        keccak_instruction = make_keccak_instruction_data(1, len(msg))

        trx_data = self.caller_ether + sign + msg
        
        trx = Transaction().add(
            TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=False),
            ])).add(
            TransactionInstruction(program_id=self.loader.loader_id, data=bytearray.fromhex("05") + trx_data, keys=[
                AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.owner_contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),  
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
            ]))
        result = send_transaction(client, trx, self.acc)

    # def test_fail_on_no_signature(self):  
    #     tx_1 = {
    #         'to': solana2ether(self.owner_contract),
    #         'value': 0,
    #         'gas': 1,
    #         'gasPrice': 1,
    #         'nonce': 0,
    #         'data': '3917b3df',
    #         'chainId': 1
    #     }
        
    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())
    #     keccak_instruction = make_keccak_instruction_data(1, len(msg))
        
    #     (caller, caller_nonce) = self.loader.ether2programAddress(from_addr)
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
    #     result = client.send_transaction(trx, self.acc.get_acc())


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
    #     result = client.send_transaction(trx, self.acc.get_acc())

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
    #     result = client.send_transaction(trx, self.acc.get_acc())


if __name__ == '__main__':
    unittest.main()
