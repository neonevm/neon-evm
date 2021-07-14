from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
import unittest
from eth_utils import abi
from base58 import b58decode

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")

class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_delete_account.py setUpClass")

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

        print("deploy contract: ")
        program_and_code = cls.loader.deployChecked(
                CONTRACTS_DIR+'SolidityPrecompilesTest.binary',
                cls.caller,
                cls.caller_ether
            )
        cls.owner_contract = program_and_code[0]
        cls.contract_code = program_and_code[2]
        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())
        print("code id: ", cls.contract_code)


    def make_transactions(self, call_data):
        eth_tx = {
            'to': solana2ether(self.owner_contract),
            'value': 0,
            'gas': 9999999,
            'gasPrice': 1,
            'nonce': getTransactionCount(client, self.caller),
            'data': call_data,
            'chainId': 111
        }

        (_from_addr, sign, msg) = make_instruction_data_from_tx(eth_tx, self.acc.secret_key())
        trx_data = self.caller_ether + sign + msg
        keccak_instruction = make_keccak_instruction_data(1, len(msg))
        
        solana_trx = Transaction().add(
                self.sol_instr_keccak(keccak_instruction) 
            ).add( 
                self.sol_instr_call(trx_data) 
            )

        return solana_trx

    def sol_instr_keccak(self, keccak_instruction):
        return  TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
                    AccountMeta(pubkey=self.caller, is_signer=False, is_writable=False),
                ])

    def sol_instr_call(self, trx_data):
        return TransactionInstruction(program_id=self.loader.loader_id, data=bytearray.fromhex("05") + trx_data, keys=[
                    AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),
                    AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
                ])

    def make_ecrecover(self, data):
        return abi.function_signature_to_4byte_selector('test_01_ecrecover(bytes32, uint8, bytes32, bytes32)')\
                + bytes.fromhex("%062x" % 0x0 + "20") \
                + bytes.fromhex("%064x" % len(data))\
                + data.to_bytes()

    def make_sha256(self, data):
        return abi.function_signature_to_4byte_selector('test_02_sha256(bytes)')\
                + bytes.fromhex("%062x" % 0x0 + "20") \
                + bytes.fromhex("%064x" % len(data))\
                + str.encode(data)

    def make_ripemd160(self, data):
        return abi.function_signature_to_4byte_selector('test_03_ripemd160(bytes)')\
                + bytes.fromhex("%062x" % 0x0 + "20") \
                + bytes.fromhex("%064x" % len(data))\
                + str.encode(data)


if __name__ == '__main__':
    unittest.main()
