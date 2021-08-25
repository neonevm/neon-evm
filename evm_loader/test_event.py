from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from spl.token.constants import TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, ACCOUNT_LEN
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")

ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))

class EventTest(unittest.TestCase):
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
            CONTRACTS_DIR+"ReturnsEvents.binary", cls.caller, cls.caller_ether)
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())
        print ('contract_code', cls.re_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

    def sol_instr_05(self, evm_instruction):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("05") + self.collateral_pool_index_buf + evm_instruction, 
            keys=[
                # Additional accounts for EvmInstruction::CallFromRawEthereumTX:
                # System instructions account:
                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                # Operator address:
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=True),
                # Collateral pool address:
                AccountMeta(pubkey=self.collateral_pool_address, is_signer=False, is_writable=True),
                # Operator ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(self.acc.public_key(), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # System program account:
                AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

                AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),
            ])

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
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

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
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ])

    def sol_instr_12_cancel(self, storage_account):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("0C"),
            keys=[
                AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),

                # Operator address:
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=True),
                # Incenirator
                AccountMeta(pubkey=PublicKey(incinerator), is_signer=False, is_writable=True),
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
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ])


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

    def call_cancel(self, storage):
        print("Cancel")
        trx = Transaction()
        trx.add(self.sol_instr_12_cancel(storage))
        return send_transaction(client, trx, self.acc)

    def get_call_parameters(self, input):
        tx = {'to': solana2ether(self.reId), 'value': 0, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller), 'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        return (from_addr, sign, msg)

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def call_signed(self, input):
        (from_addr, sign,  msg) = self.get_call_parameters(input)

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 5)))
        trx.add(self.sol_instr_05(from_addr + sign + msg))
        return send_transaction(client, trx, self.acc)["result"]

    def create_storage_account(self, seed):
        storage = PublicKey(sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)

        return storage

    def call_partial_signed(self, input):
        (from_addr, sign,  msg) = self.get_call_parameters(input)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())
        self.call_begin(storage, 10, msg, instruction)

        while (True):
            result = self.call_continue(storage, 50)["result"]

            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    return result


    def test_addNoReturn(self):
        func_name = abi.function_signature_to_4byte_selector('addNoReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2) )
        calls = [ (self.call_signed, 1), (self.call_partial_signed, 0) ]
        for (call, index) in calls:
            with self.subTest(call.__name__):
                result = call(input)
                self.assertEqual(result['meta']['err'], None)
                self.assertEqual(len(result['meta']['innerInstructions']), 1)
                # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                self.assertEqual(data[0], 6)  # 6 means OnReturn,
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(int().from_bytes(data[2:10], 'little'), 21657) # used_gas

    def test_addReturn(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        calls = [ (self.call_signed, 1), (self.call_partial_signed, 0) ]
        for (call, index) in calls:
            with self.subTest(call.__name__):
                result = call(input)
                self.assertEqual(result['meta']['err'], None)
                self.assertEqual(len(result['meta']['innerInstructions']), 1)
                # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(int().from_bytes(data[2:10], 'little'), 21719) # used_gas
                self.assertEqual(data[10:], bytes().fromhex("%064x" % 0x3))

    def test_addReturnEvent(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEvent(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        calls = [ (self.call_signed, 1), (self.call_partial_signed, 0) ]
        for (call, index) in calls:
            with self.subTest(call.__name__):
                result = call(input)
                self.assertEqual(result['meta']['err'], None)
                self.assertEqual(len(result['meta']['innerInstructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-2]['data'])
                self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(int().from_bytes(data[2:10], 'little'), 22743) # used_gas
                self.assertEqual(data[10:42], bytes().fromhex('%064x' % 3)) # sum

    def test_addReturnEventTwice(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEventTwice(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        calls = [ (self.call_signed, 1), (self.call_partial_signed, 0) ]
        for (call, index) in calls:
            with self.subTest(call.__name__):
                result = call(input)
                self.assertEqual(result['meta']['err'], None)
                self.assertEqual(len(result['meta']['innerInstructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-3]['data'])
                # self.assertEqual(data[:1], b'\x07')
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-2]['data'])
                self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x5))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(int().from_bytes(data[2:10], 'little'), 23858) # used_gas
                self.assertEqual(data[10:42], bytes().fromhex('%064x' % 5)) # sum

    def test_events_of_different_instructions(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEventTwice(uint8,uint8)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        input2 = (func_name + bytes.fromhex("%064x" % 0x3) + bytes.fromhex("%064x" % 0x4))
        tx1 =  {'to': solana2ether(self.reId), 'value': 0, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller), 'data': input1, 'chainId': 111}
        tx2 =  {'to': solana2ether(self.reId), 'value': 0, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller)+1, 'data': input2, 'chainId': 111}

        (from_addr1, sign1, msg1) = make_instruction_data_from_tx(tx1, self.acc.secret_key())
        (from_addr2, sign2, msg2) = make_instruction_data_from_tx(tx2, self.acc.secret_key())
        assert (from_addr1 == self.caller_ether)
        assert (from_addr2 == self.caller_ether)

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg1), 5)))
        trx.add(self.sol_instr_05(from_addr1 + sign1 + msg1))
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(3, len(msg2), 5)))
        trx.add(self.sol_instr_05(from_addr2 + sign2 + msg2))

        result = send_transaction(client, trx, self.acc)["result"]

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 2) # two transaction-instructions contain events and return_value

        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
        self.assertEqual(result['meta']['innerInstructions'][1]['index'], 3)  # second instruction

        # log sol_instr_05(from_addr1 + sign1 + msg1)
        # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-3]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-2]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x5))  # sum
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(int().from_bytes(data[2:10], 'little'), 23858) # used_gas
        self.assertEqual(data[10:42], bytes().fromhex('%064x' % 0x5)) # sum

        # log sol_instr_05(from_addr2 + sign2 + msg2)
        # self.assertEqual(len(result['meta']['innerInstructions'][1]['instructions']), 3)
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][-3]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x7))  # sum
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][-2]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0xb))  # sum
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][-1]['data'])
        self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(int().from_bytes(data[2:10], 'little'), 23858) # used_gas
        self.assertEqual(data[10:42], bytes().fromhex('%064x' % 0xb)) # sum

    def test_caseFailAfterCancel(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr, sign,  msg) = self.get_call_parameters(input)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())

        result = self.call_begin(storage, 10, msg, instruction)
        result = self.call_continue(storage, 10)
        result = self.call_cancel(storage)
            
        err = "custom program error: 0x1"
        with self.assertRaisesRegex(Exception,err):
            result = self.call_continue(storage, 10)
            print(result)


    def test_caseSuccessRunOtherTransactionAfterCancel(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr, sign,  msg) = self.get_call_parameters(input)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())

        caller_balance_before_cancel = self.token.balance(self.caller_token)

        result = self.call_begin(storage, 10, msg, instruction)
        result = self.call_continue(storage, 10)
        result = self.call_cancel(storage)

        caller_balance_after_cancel = self.token.balance(self.caller_token)
        self.assertEqual(caller_balance_after_cancel, caller_balance_before_cancel)

        self.call_partial_signed(input)


if __name__ == '__main__':
    unittest.main()
