from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"


class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(solana_url, wallet, evm_loader_id)
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

        (cls.reId, cls.reId_eth, cls.re_code) = cls.loader.deployChecked(CONTRACTS_DIR+"ReturnsEvents.binary", solana2ether(cls.acc.public_key()))
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())
        print ('contract_code', cls.re_code)

    def sol_instr_05(self, evm_instruction):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_09_partial_call(self, storage_account, step_count, evm_instruction):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("09") + step_count.to_bytes(8, byteorder='little') + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_10_continue(self, storage_account, step_count):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_12_cancel(self, storage_account):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("0C"),
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])


    def call_begin(self, storage, steps, msg, instruction):
        print("Begin")
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 9)))
        trx.add(self.sol_instr_09_partial_call(storage, steps, instruction))
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))
        return result

    def call_continue(self, storage, steps):
        print("Continue")
        trx = Transaction()
        trx.add(self.sol_instr_10_continue(storage, steps))
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))
        return result

    def call_cancel(self, storage):
        print("Cancel")
        trx = Transaction()
        trx.add(self.sol_instr_12_cancel(storage))
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))
        return result

    def get_call_parameters(self, input):
        tx = {'to': solana2ether(self.reId), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)

        return (from_addr, sign, msg)


    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def call_signed(self, input):
        (from_addr, sign,  msg) = self.get_call_parameters(input)

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
        trx.add(self.sol_instr_05(from_addr + sign + msg))
        return http_client.send_transaction(trx, self.acc,
                                     opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]

    def create_storage_account(self, seed):
        storage = PublicKey(sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False))

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
                self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
                self.assertEqual(data[0], 6)  # 6 means OnReturn,
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success

    def test_addReturn(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        calls = [ (self.call_signed, 1), (self.call_partial_signed, 0) ]
        for (call, index) in calls:
            with self.subTest(call.__name__):
                result = call(input)
                self.assertEqual(result['meta']['err'], None)
                self.assertEqual(len(result['meta']['innerInstructions']), 1)
                self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
                self.assertEqual(result['meta']['innerInstructions'][0]['index'], index)  # second instruction
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
                self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(data[2:], bytes().fromhex("%064x" % 0x3))

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
                self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
                self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
                self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(data[2:34], bytes().fromhex('%064x' % 3)) #sum

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
                self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
                # self.assertEqual(data[:1], b'\x07')
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
                self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
                self.assertEqual(data[1:21], self.reId_eth)
                self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
                self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
                self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x5))  # sum
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][2]['data'])
                self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
                self.assertLess(data[1], 0xd0)  # less 0xd0 - success
                self.assertEqual(data[2:34], bytes().fromhex('%064x' % 5)) #sum

    def test_events_of_different_instructions(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEventTwice(uint8,uint8)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        input2 = (func_name + bytes.fromhex("%064x" % 0x3) + bytes.fromhex("%064x" % 0x4))
        tx1 =  {'to': solana2ether(self.reId), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input1, 'chainId': 1}
        tx2 =  {'to': solana2ether(self.reId), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller)+1, 'data': input2, 'chainId': 1}

        (from_addr1, sign1, msg1) = make_instruction_data_from_tx(tx1, self.acc.secret_key())
        (from_addr2, sign2, msg2) = make_instruction_data_from_tx(tx2, self.acc.secret_key())
        assert (from_addr1 == self.caller_ether)
        assert (from_addr2 == self.caller_ether)

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg1))))
        trx.add(self.sol_instr_05(from_addr1 + sign1 + msg1))
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(3, len(msg2))))
        trx.add(self.sol_instr_05(from_addr2 + sign2 + msg2))
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 2) # two transaction-instructions contain events and return_value

        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
        self.assertEqual(result['meta']['innerInstructions'][1]['index'], 3)  # second instruction

        # log sol_instr_05(from_addr1 + sign1 + msg1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x5))  # sum
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][2]['data'])
        self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(data[2:34], bytes().fromhex('%064x' % 0x5)) #sum

        # log sol_instr_05(from_addr2 + sign2 + msg2)
        self.assertEqual(len(result['meta']['innerInstructions'][1]['instructions']), 3)
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x7))  # sum
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0xb))  # sum
        data = b58decode(result['meta']['innerInstructions'][1]['instructions'][2]['data'])
        self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(data[2:34], bytes().fromhex('%064x' % 0xb)) #sum


    def test_caseFailAfterCancel(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr, sign,  msg) = self.get_call_parameters(input)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())

        result = self.call_begin(storage, 10, msg, instruction)
        result = self.call_continue(storage, 10)
        result = self.call_cancel(storage)
            
        err = "invalid account data for instruction"
        with self.assertRaisesRegex(Exception,err):
            result = self.call_continue(storage, 10)
            print(result)


    def test_caseSuccessRunOtherTransactionAfterCancel(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr, sign,  msg) = self.get_call_parameters(input)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())

        result = self.call_begin(storage, 10, msg, instruction)
        result = self.call_continue(storage, 10)
        result = self.call_cancel(storage)

        self.call_partial_signed(input)


if __name__ == '__main__':
    unittest.main()
