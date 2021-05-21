import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, Trx
from eth_utils import abi
from web3.auto import w3
from eth_keys import keys
from web3 import Web3


solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")


sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"


class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
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

        (cls.reId_caller, cls.reId_caller_eth, cls.reId_caller_code) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Caller.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_reciever, cls.reId_reciever_eth, cls.reId_reciever_code) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Receiver.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_recover, cls.reId_recover_eth, cls.reId_recover_code) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Recover.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_create_caller, cls.reId_create_caller_eth, cls.reId_create_caller_code) = cls.loader.deployChecked(CONTRACTS_DIR+"Create_Caller.binary", solana2ether(cls.acc.public_key()))
        (cls.reId_revert, cls.reId_revert_eth, cls.reId_revert_code) = cls.loader.deployChecked(CONTRACTS_DIR+"nested_call_Revert.binary", solana2ether(cls.acc.public_key()))
        print ('reId_contract_caller', cls.reId_caller)
        print ('reId_contract_caller_eth', cls.reId_caller_eth.hex())
        print ('reId_contract_reciever', cls.reId_reciever)
        print ('reId_contract_receiver_eth', cls.reId_reciever_eth.hex())
        print ('reId_contract_recover', cls.reId_recover)
        print ('reId_contract_recover_eth', cls.reId_recover_eth.hex())
        print ('reId_contract_create_caller', cls.reId_create_caller)
        print ('reId_contract_create_caller_eth', cls.reId_create_caller_eth.hex())
        print ('reId_contract_revert', cls.reId_revert)
        print ('reId_contract_revert_eth', cls.reId_revert_eth.hex())

        with open(CONTRACTS_DIR+"Create_Receiver.binary", mode='rb') as file:
            fileHash = Web3.keccak(file.read())
            cls.reId_create_receiver_eth = bytes(Web3.keccak(b'\xff' + cls.reId_create_caller_eth + bytes(32) + fileHash)[-20:])
        (cls.reId_create_receiver, _) = cls.loader.ether2program(cls.reId_create_receiver_eth)
        print ("reId_create_receiver", cls.reId_create_receiver)
        print ("reId_create_receiver_eth", cls.reId_create_receiver_eth.hex())

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def sol_instr_09_partial_call(self, storage_account, step_count, evm_instruction, contract, code):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("09") + step_count.to_bytes(8, byteorder='little') + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_caller_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_10_continue(self, storage_account, step_count, evm_instruction, contract, code):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little') + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_caller_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])
    def create_storage_account(self, seed):
        storage = PublicKey(sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(self.loader.loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)

        return storage

    def call_partial_signed(self, input, contract, code):
        tx = {'to': solana2ether(contract), 'value': 1, 'gas': 1, 'gasPrice': 1,
            'nonce': getTransactionCount(client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        instruction = from_addr + sign + msg

        storage = self.create_storage_account(sign[:8].hex())

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 9)))
        trx.add(self.sol_instr_09_partial_call(storage, 400, instruction, contract, code))
        send_transaction(client, trx, self.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 9)))
            trx.add(self.sol_instr_10_continue(storage, 400, instruction, contract, code))
            result = send_transaction(client, trx, self.acc)["result"]

            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    return result


    def test_callFoo(self):
        func_name = abi.function_signature_to_4byte_selector('callFoo(address)')
        data = (func_name + bytes.fromhex("%024x" % 0x0 + self.reId_reciever_eth.hex()))
        result = self.call_partial_signed(input=data, contract=self.reId_caller, code=self.reId_caller_code)
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
              'nonce': getTransactionCount(client, self.caller), 'data': bytes().fromhex("001122"), 'chainId': 111}

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
        # result = self.call_signed(input=data, contract=self.reId_caller)
        result = self.call_partial_signed(input=data, contract=self.reId_caller, code=self.reId_caller_code)
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


    def test_create2_opcode(self):
        func_name = abi.function_signature_to_4byte_selector('creator()')
        result = self.call_partial_signed(input=func_name, contract=self.reId_create_caller, code=self.reId_create_caller_code)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3) # TODO: why not 2?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction

        # emit Foo(caller, amount, message)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_create_receiver_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Foo(address,uint256,string)'))
        self.assertEqual(data[61:93], bytes.fromhex("%024x" %0x0 + self.reId_create_caller_eth.hex()))
        self.assertEqual(data[93:125], bytes.fromhex("%064x" %0x0))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "60"))
        self.assertEqual(data[157:189], bytes.fromhex("%062x" %0x0 + "08"))
        s = "call foo".encode("utf-8")
        self.assertEqual(data[189:221], bytes.fromhex('{:0<64}'.format(s.hex())))

        # emit Result_foo(result)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_create_caller_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Result_foo(uint256)'))
        self.assertEqual(data[61:93], bytes.fromhex("%062x" %0x0 + hex(124)[2:]))

    def test_nested_revert(self):
        func_name = abi.function_signature_to_4byte_selector('callFoo(address)')
        data = (func_name + bytes.fromhex("%024x" % 0x0 + self.reId_revert_eth.hex()))
        result = self.call_partial_signed(input=data, contract=self.reId_caller, code=self.reId_caller_code)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)  # TODO: why not 1?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 1)  # second instruction

        #  emit Result(success, data);
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07') # 7 means OnEvent
        self.assertEqual(data[1:21], self.reId_caller_eth)
        count_topics = int().from_bytes(data[21:29], 'little')
        self.assertEqual(count_topics, 1)
        self.assertEqual(data[29:61], abi.event_signature_to_log_topic('Result(bool,bytes)'))
        self.assertEqual(data[61:93], bytes.fromhex("%062x" %0x0 + "00")) # result false
        self.assertEqual(data[93:125], bytes.fromhex("%062x" %0x0 + "40"))
        self.assertEqual(data[125:157], bytes.fromhex("%062x" %0x0 + "00"))

