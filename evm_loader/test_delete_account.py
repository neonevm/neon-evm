from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
import base58
from eth_account import Account as EthAccount

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
    
    def deploy_contract(self):
        print("deploy contract: ")
        program_and_code = self.loader.deployChecked(
                CONTRACTS_DIR+'SelfDestructContract.binary',
                self.caller,
                self.caller_ether
            )
        owner_contract = program_and_code[0]
        contract_code = program_and_code[2]
        print("contract id: ", owner_contract, solana2ether(owner_contract).hex())
        print("code id: ", contract_code)
        return (owner_contract, contract_code)

    def make_transaction(self, owner_contract, nonce, position):
        tx = {
            'to': solana2ether(owner_contract),
            'value': 1,
            'gas': 1,
            'gasPrice': 1,
            'nonce': nonce,
            'data': '183555f0',
            'chainId': 111
        }
        (_from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        trx_data = self.caller_ether + sign + msg
        keccak_instruction = make_keccak_instruction_data(position, len(msg))
        return (trx_data, keccak_instruction)

    def sol_instr_keccak(self, keccak_instruction):
        return  TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
                    AccountMeta(pubkey=self.caller, is_signer=False, is_writable=False),
                ])
    
    def sol_instr_call(self, trx_data, owner_contract, contract_code):
        return TransactionInstruction(program_id=self.loader.loader_id, data=bytearray.fromhex("05") + trx_data, keys=[
                    AccountMeta(pubkey=owner_contract, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=contract_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=PublicKey("Sysvar1nstructions1111111111111111111111111"), is_signer=False, is_writable=False),
                    AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
                ])


    def test_success_tx_send(self):
        (owner_contract, contract_code) = self.deploy_contract()

        init_nonce = getTransactionCount(client, self.caller)
        (trx_data_1, keccak_instruction_1) = self.make_transaction(owner_contract, init_nonce, 1)
        init_nonce += 1
        (trx_data_2, keccak_instruction_2) = self.make_transaction(owner_contract, init_nonce, 3)

        trx = Transaction().add(
                self.sol_instr_keccak(keccak_instruction_1)
            ).add(
                self.sol_instr_call(trx_data_1, owner_contract, contract_code)
            ).add(
                self.sol_instr_keccak(keccak_instruction_2)
            ).add(
                self.sol_instr_call(trx_data_2, owner_contract, contract_code)
            )

        err = "invalid account data for instruction"
        with self.assertRaisesRegex(Exception,err):
            result = send_transaction(client, trx, self.acc)
            print(result)


    def test_funds_transfer(self):
        (owner_contract, contract_code) = self.deploy_contract()

        init_nonce = getTransactionCount(client, self.caller)
        (trx_data_1, keccak_instruction_1) = self.make_transaction(owner_contract, init_nonce, 1)

        caller_balance_pre = getBalance(self.caller)
        contract_balance_pre = getBalance(owner_contract)

        trx = Transaction().add(
                self.sol_instr_keccak(keccak_instruction_1)
            ).add(
                self.sol_instr_call(trx_data_1, owner_contract, contract_code)
            )

        result = send_transaction(client, trx, self.acc)

        caller_balance_post = getBalance(self.caller)
        contract_balance_post = getBalance(owner_contract)

        self.assertEqual(caller_balance_post, contract_balance_pre + caller_balance_pre)
        self.assertEqual(contract_balance_post, 0)


if __name__ == '__main__':
    unittest.main()
