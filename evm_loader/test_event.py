from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
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

        (cls.reId, cls.reId_eth) = cls.loader.deployChecked("event.bin", solana2ether(cls.acc.public_key()))
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())

    def sol_instr_05(self, evm_instruction):
        return TransactionInstruction(program_id=evm_loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def call_signed(self, input):
        tx = {'to': solana2ether(self.reId), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input, 'chainId': 1}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
        trx.add(self.sol_instr_05(from_addr + sign + msg))
        return http_client.send_transaction(trx, self.acc,
                                     opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]

    def test_addNoReturn(self):
        func_name = abi.function_signature_to_4byte_selector('addNoReturn(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2) )
        result = self.call_signed(input=data)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data, b'\x06')  # 6 means OnReturn, and no value next - empty

    def test_addReturn(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        result = self.call_signed(input=data)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x06')
        self.assertEqual(data[1:], bytes().fromhex("%064x" % 0x3))

    def test_addReturnEvent(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEvent(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        result = self.call_signed(input=data)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_eth)
        self.assertEqual(data[21:29], bytes().fromhex('%016x' % 1)[::-1])  # topics len
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Added(uint8)'))  #topics
        self.assertEqual(data[61:93], bytes().fromhex("%064x" % 0x3))  # sum
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x06')   # 6 means OnReturn
        self.assertEqual(data[1:33], bytes().fromhex('%064x' % 3)) #sum

    def test_addReturnEventTwice(self):
        func_name = abi.function_signature_to_4byte_selector('addReturnEventTwice(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x2))
        result = self.call_signed(input=data)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
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
        self.assertEqual(data[1:33], bytes().fromhex('%064x' % 5)) #sum

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
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg2))))
        trx.add(self.sol_instr_05(from_addr2 + sign2 + msg2))
        result = http_client.send_transaction(trx, self.acc,
                                     opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
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
        self.assertEqual(data[1:33], bytes().fromhex('%064x' % 0x5)) #sum

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
        self.assertEqual(data[1:33], bytes().fromhex('%064x' % 0xb)) #sum

if __name__ == '__main__':
    unittest.main()