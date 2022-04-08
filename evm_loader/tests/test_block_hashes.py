from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from spl.token.instructions import get_associated_token_address
import unittest
from eth_utils import abi
from base58 import b58decode
import random

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/tests")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
holder_id = 0

class BlockHashesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_block_hashes.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet = OperatorAccount(operator1_keypair_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            # cls.token.transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID))
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        print("deploy contract: ")
        (cls.owner_contract, cls.eth_contract, cls.contract_code) = cls.loader.deployChecked(
                CONTRACTS_DIR+'BlockHashTest.binary',
                cls.caller,
                cls.caller_ether
            )
        print("contract id: ", cls.owner_contract, cls.eth_contract.hex())
        print("code id: ", cls.contract_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

    def send_transaction(self, data, no_sys_acc = False):
        trx = self.make_transactions(data, no_sys_acc)
        result = send_transaction(client, trx, self.acc)
        result = result["result"]
        return b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])[8+2:].hex()

    def make_transactions(self, call_data, no_sys_acc) -> Transaction:
        eth_tx = {
            'to': self.eth_contract,
            'value': 0,
            'gas': 999999999,
            'gasPrice': 0,
            'nonce': getTransactionCount(client, self.caller),
            'data': call_data,
            'chainId': 111
        }

        (_from_addr, sign, msg) = make_instruction_data_from_tx(eth_tx, self.acc.secret_key())
        trx_data = self.caller_ether + sign + msg

        solana_trx = TransactionWithComputeBudget()
        solana_trx.add(
                self.sol_instr_keccak(make_keccak_instruction_data(len(solana_trx.instructions) + 1, len(msg), 5)) 
            ).add( 
                self.sol_instr_call(trx_data, no_sys_acc) 
            )

        return solana_trx

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                    AccountMeta(pubkey=keccakprog, is_signer=False, is_writable=False),
                ])

    def sol_instr_call(self, trx_data, no_sys_acc):
        blockhash_sysvar_accountmeta = AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False)
        neon_evm_instr_05_single = create_neon_evm_instr_05_single(
            self.loader.loader_id,
            self.caller,
            self.acc.public_key(),
            self.owner_contract,
            self.contract_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            trx_data,
            add_meta=[blockhash_sysvar_accountmeta,] if not no_sys_acc else []
        )
        return neon_evm_instr_05_single

    def make_getCurrentValues(self):
        return abi.function_signature_to_4byte_selector('getCurrentValues()')

    def make_getValues(self, number: int):
        return abi.function_signature_to_4byte_selector('getValues(uint256)')\
                + bytes.fromhex("%064x" % number)

    def emulate_call(self, call_data):
        cmd = "{} {} {} {}".format(self.caller_ether.hex(), self.eth_contract.hex(), call_data.hex(), "")
        output = neon_cli().emulate(evm_loader_id, cmd)
        result = json.loads(output)
        return result

    def get_blocks_from_solana(self):
        '''
        Get slot hash history of last 100 blocks
        '''
        slot_hash = {}
        current_slot = client.get_slot()["result"]
        for slot in range(max(current_slot - 100, 0), current_slot):
            hash_val = base58.b58decode(client.get_confirmed_block(slot)['result']['blockhash']).hex()
            slot_hash[int(slot)] = hash_val
        return slot_hash

    def test_01_get_block_hashes(self):
        '''
        Recent blockhashes store history of last 150 blocks. Get block hashes within them.
        '''
        print("test_01_get_block_hashes")
        solana_result = self.get_blocks_from_solana()
        for _ in range(3):
            sol_slot, sol_hash = random.choice(list(solana_result.items()))
            result_hash = self.send_transaction(self.make_getValues(sol_slot))
            emulate_hash = self.emulate_call(self.make_getValues(sol_slot))['result']
            self.assertEqual(sol_hash, result_hash)
            self.assertEqual(sol_hash, emulate_hash)

    def test_02_get_current_block_hashes(self):
        '''
        Solana doesn't have current block hash at execution state, so it will return default hash
        '''
        print("test_02_get_current_block_hashes")
        DEFAULT_ZERO_HASH = '0000000000000000000000000000000000000000000000000000000000000000'
        result_hash = self.send_transaction(self.make_getCurrentValues())
        emulate_hash = self.emulate_call(self.make_getCurrentValues())['result']
        self.assertEqual(result_hash, DEFAULT_ZERO_HASH)
        self.assertEqual(emulate_hash, DEFAULT_ZERO_HASH)

    def test_03_fail_on_no_sysvar_account(self):
        '''
        Must fail if no sysvar blockhashes account provided
        '''
        print("test_03_fail_on_no_sysvar_account")
        err = "Program failed to complete"
        with self.assertRaisesRegex(Exception,err):
            self.send_transaction(self.make_getCurrentValues(), True)


if __name__ == '__main__':
    unittest.main()