from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from spl.token.constants import TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi
from decimal import Decimal

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"

ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))


class EthTokenTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_event.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            cls.token.transfer(ETH_TOKEN_MINT_ID, 2000, get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID))
            print("Done\n")

        cls.caller_holder = get_caller_hold_token(cls.loader, cls.acc, cls.caller_ether)

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        (cls.reId, cls.reId_eth, cls.re_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"EthToken.binary", cls.caller, cls.caller_ether)
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())
        print ('contract_code', cls.re_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

    def sol_instr_09_partial_call(self, storage_account, step_count, evm_instruction):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("09") + self.collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little') + evm_instruction,
            keys=[
                AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),

                # System instructions account:
                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                # Operator address:
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=True),
                # Collateral pool address:
                AccountMeta(pubkey=self.collateral_pool_address, is_signer=False, is_writable=True),
                # Operator ETH address (stub for now):
                AccountMeta(pubkey=self.caller_holder, is_signer=False, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # System program account:
                AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

                AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller_token, is_signer=False, is_writable=True),

                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ])

    def sol_instr_10_continue(self, storage_account, step_count):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
            keys=[
                AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),

                # Operator address:
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(self.acc.public_key(), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # Operator ETH address (stub for now):
                AccountMeta(pubkey=self.caller_holder, is_signer=False, is_writable=True),
                # System program account:
                AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

                AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller_token, is_signer=False, is_writable=True),

                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ])

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def call_begin(self, storage, steps, msg, instruction):
        print("Begin")
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 13)))
        trx.add(self.sol_instr_09_partial_call(storage, steps, instruction))
        return send_transaction(client, trx, self.acc)

    def call_continue(self, storage, steps):
        print("Continue")
        trx = Transaction()
        trx.add(self.sol_instr_10_continue(storage, steps))
        return send_transaction(client, trx, self.acc)

    def get_call_parameters(self, input, value):
        tx = {'to': solana2ether(self.reId), 'value': value, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller), 'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)

        return (from_addr, sign, msg)

    def create_storage_account(self, seed):
        storage = PublicKey(sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)

        return storage

    def call_partial_signed(self, input, value):
        (from_addr, sign,  msg) = self.get_call_parameters(input, value)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())
        result = self.call_begin(storage, 0, msg, instruction)

        while (True):
            result = self.call_continue(storage, 400)["result"]

            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    return result


    def test_caller_balance(self):
        expected_balance = self.token.balance(self.caller_token)

        func_name = abi.function_signature_to_4byte_selector('checkCallerBalance(uint256)')
        input = func_name + bytes.fromhex("%064x" % int(expected_balance * 10**18))
        result = self.call_partial_signed(input, 0)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
        self.assertEqual(data[1], 0x11)  #  0x11 - stoped

    def test_contract_balance(self):
        contract_token = get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID)
        expected_balance = self.token.balance(contract_token)

        func_name = abi.function_signature_to_4byte_selector('checkContractBalance(uint256)')
        input = func_name + bytes.fromhex("%064x" % int(expected_balance * (10**18)))
        result = self.call_partial_signed(input, 0)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
        self.assertEqual(data[1], 0x11)  #  0x11 - stoped

    def test_transfer_and_call(self):
        contract_token = get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID)

        contract_balance_before = self.token.balance(contract_token)
        caller_balance_before = self.token.balance(self.caller_token)
        value = 10

        func_name = abi.function_signature_to_4byte_selector('nop()')
        result = self.call_partial_signed(func_name, value * (10**18))

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 4)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
        self.assertEqual(data[1], 0x11)  #  0x11 - stoped

        gas_used = Decimal(int().from_bytes(data[2:10],'little'))/Decimal(1_000_000_000)

        contract_balance_after = self.token.balance(contract_token)
        caller_balance_after = self.token.balance(self.caller_token)
        self.assertEqual(contract_balance_after, contract_balance_before + value)
        self.assertEqual(caller_balance_after, caller_balance_before - value - gas_used)

    def test_transfer_internal(self):
        contract_token = get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID)
        self.token.transfer(ETH_TOKEN_MINT_ID, 500, contract_token)

        contract_balance_before = self.token.balance(contract_token)
        caller_balance_before = self.token.balance(self.caller_token)
        value = 5
        func_name = abi.function_signature_to_4byte_selector('retrieve(uint256)')
        input = func_name + bytes.fromhex("%064x" % (value * (10**18)))
        result = self.call_partial_signed(input, 0)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 4)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
        self.assertEqual(data[1], 0x11)  #  0x11 - stoped

        gas_used = Decimal(int().from_bytes(data[2:10],'little'))/Decimal(1_000_000_000)

        contract_balance_after = self.token.balance(contract_token)
        caller_balance_after = self.token.balance(self.caller_token)
        self.assertEqual(contract_balance_after, contract_balance_before - value)
        self.assertEqual(caller_balance_after, caller_balance_before + value - gas_used)

if __name__ == '__main__':
    unittest.main()
