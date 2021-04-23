import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, Trx
from eth_utils import abi
from web3.auto import w3
from eth_keys import keys

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
# CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "")
evm_loader_id = os.environ.get("EVM_LOADER")
# evm_loader_id = "Gw3fK17P5HsZ3titT139SnmBF9cwuYEBb4zwUUgfT2Ua"
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

        (cls.reId_caller, cls.reId_caller_eth) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Caller.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_reciever, cls.reId_reciever_eth) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Receiver.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_recover, cls.reId_recover_eth) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Recover.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_create_caller, cls.reId_create_caller_eth) = cls.loader.deployChecked(CONTRACTS_DIR+"Create_Caller.binary", solana2ether(cls.acc.public_key()))
        print ('contract_caller', cls.reId_caller)
        print ('contract_caller_eth', cls.reId_caller_eth.hex())
        print ('contract_reciever', cls.reId_reciever)
        print ('contract_receiver_eth', cls.reId_reciever_eth.hex())
        print ('contract_recover', cls.reId_recover)
        print ('contract_recover_eth', cls.reId_recover_eth.hex())
        print ('contract_create_caller', cls.reId_create_caller)
        print ('contract_create_caller_eth', cls.reId_create_caller_eth.hex())

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def sol_instr_05(self, evm_instruction, contract):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def call_signed(self, input, contract):
        tx = {'to': solana2ether(self.reId_caller), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
        trx.add(self.sol_instr_05(from_addr + sign + msg, contract))
        return http_client.send_transaction(trx, self.acc,
                                     opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]


    def test_callFoo(self):
        func_name = abi.function_signature_to_4byte_selector('callFoo(address)')
        data = (func_name + bytes.fromhex("%024x" % 0x0 + self.reId_reciever_eth.hex()))
        result = self.call_signed(input=data, contract=self.reId_caller)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3) # TODO: why not 2?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction

        #  emit Foo(msg.sender, msg.value, _message);
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_reciever_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Foo(address,uint256,string)'))
        self.assertEqual(data[61:93], bytes.fromhex("%024x" %0x0 + self.reId_caller_eth.hex()))
        self.assertEqual(data[93:125], bytes.fromhex("%064x" %0x0))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "60"))
        self.assertEqual(data[157:189], bytes.fromhex("%062x" %0x0 + "08"))
        s = "call foo".encode("utf-8")
        self.assertEqual(data[189:221], bytes.fromhex('{:0<64}'.format(s.hex())))

        # emit Result(success, data);
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_caller_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Result(bool,bytes)'))
        self.assertEqual(data[61:93], bytes.fromhex("%062x" %0x0 + "01"))
        self.assertEqual(data[93:125], bytes.fromhex("%062x" %0x0 + "40"))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "20"))
        self.assertEqual(data[157:189], bytes.fromhex("%062x" %0x0 + hex(124)[2:]))

    def test_ecrecover(self):
        tx = {'to': solana2ether(self.reId_caller), 'value': 1, 'gas': 1, 'gasPrice': 1,
              'nonce': getTransactionCount(http_client, self.caller), 'data': bytes().fromhex("001122"), 'chainId': 111}

        signed_tx = w3.eth.account.sign_transaction(tx, self.acc.secret_key())
        _trx = Trx.fromString(signed_tx.rawTransaction)
        sig = keys.Signature(vrs=[1 if _trx.v%2==0 else 0, _trx.r, _trx.s])

        func_name = abi.function_signature_to_4byte_selector('callRecover(address,address,bytes32,bytes)')
        data = (func_name +
                bytes.fromhex("%024x" % 0x0 + self.reId_reciever_eth.hex()) +
                bytes.fromhex("%024x" % 0x0 + self.reId_recover_eth.hex()) +
                _trx.hash() +
                bytes.fromhex("%062x" % 0x0 + "80") +
                bytes.fromhex("%062x" % 0x0 + "41") +
                sig.to_bytes()
                )
        result = self.call_signed(input=data, contract=self.reId_caller)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 4) # TODO: why not 3?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction

        #  emit Recovered(address);
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_recover_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Recovered(address)'))
        self.assertEqual(data[61:93], bytes.fromhex("%024x" %0x0 + self.caller_ether.hex()))

        # emit Response_recovery_signer(success, data));
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_reciever_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Response_recovery_signer(bool,bytes)'))
        self.assertEqual(data[61:93], bytes.fromhex("%062x" %0x0 + "01"))
        self.assertEqual(data[93:125], bytes.fromhex("%062x" %0x0 + "40"))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "20"))
        self.assertEqual(data[157:189], bytes.fromhex("%024x" %0x0 + self.caller_ether.hex()))

        #  emit Result(success, data);
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][2]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_caller_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Result(bool,bytes)'))
        self.assertEqual(data[61:93], bytes.fromhex("%062x" %0x0 + "01"))
        self.assertEqual(data[93:125], bytes.fromhex("%062x" %0x0 + "40"))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "20"))
        self.assertEqual(data[157:189], bytes.fromhex("%062x" %0x0 + "01"))

    # def test_create_opcode(self):
    #     func_name = abi.function_signature_to_4byte_selector('call()')
    #     result = self.call_signed(input=func_name, contract=self.reId_create_caller)
    #     self.assertEqual(result['meta']['err'], None)
    #     self.assertEqual(len(result['meta']['innerInstructions']), 1)
    #     self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3) # TODO: why not 2?
    #     self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction
