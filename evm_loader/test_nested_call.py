import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, Trx
from spl.token.instructions import get_associated_token_address
from eth_utils import abi
from web3.auto import w3
from eth_keys import keys
from web3 import Web3


solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
# CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
evm_loader_id = os.environ.get("EVM_LOADER")
# evm_loader_id = "7NXfEKTMhPdkviCjWipXxUtkEMDRzPJMQnz39aRMCwb1"

sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"


class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_nested_call.py setUpClass")

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

        (cls.reId_caller, cls.reId_caller_eth, cls.reId_caller_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"nested_call_Caller.binary", cls.caller, cls.caller_ether)
        (cls.reId_reciever, cls.reId_reciever_eth, cls.reId_reciever_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"nested_call_Receiver.binary", cls.caller, cls.caller_ether)
        (cls.reId_recover, cls.reId_recover_eth, cls.reId_recover_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"nested_call_Recover.binary", cls.caller, cls.caller_ether)
        (cls.reId_create_caller, cls.reId_create_caller_eth, cls.reId_create_caller_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"Create_Caller.binary", cls.caller, cls.caller_ether)
        (cls.reId_revert, cls.reId_revert_eth, cls.reId_revert_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"nested_call_Revert.binary", cls.caller, cls.caller_ether)
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

        cls.reId_create_receiver_seed = b58encode(bytes.fromhex(cls.reId_create_receiver_eth.hex())).decode('utf8')
        cls.reId_create_receiver_code_account = accountWithSeed(cls.acc.public_key(), cls.reId_create_receiver_seed, PublicKey(evm_loader_id))


    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def sol_instr_11_partial_call_from_account(self, holder_account, storage_account, step_count, contract, code):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("0B") + step_count.to_bytes(8, byteorder='little'),
                                   keys=[
                                       AccountMeta(pubkey=holder_account, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_caller_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_reciever), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_recover), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_create_receiver), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver_code_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_revert), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_09_partial_call(self, storage_account, step_count, evm_instruction, contract, code):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("09") + step_count.to_bytes(8, byteorder='little') + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_caller_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_reciever), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_recover), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_create_receiver), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver_code_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_revert), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def sol_instr_10_continue(self, storage_account, step_count, contract, code):
        return TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
                                   keys=[
                                       AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.reId_caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_caller_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_reciever), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_reciever_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_recover), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_recover_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_create_receiver), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_create_receiver_code_account, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId_revert), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.reId_revert_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ])

    def create_account_with_seed(self, seed):
        storage = accountWithSeed(self.acc.public_key(), seed, PublicKey(evm_loader_id))

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False))

        return storage

    def write_transaction_to_holder_account(self, holder, signature, message):
        message = signature + len(message).to_bytes(8, byteorder="little") + message

        offset = 0
        receipts = []
        rest = message
        while len(rest):
            (part, rest) = (rest[:1000], rest[1000:])
            trx = Transaction()
            trx.add(TransactionInstruction(program_id=evm_loader_id,
                data=(bytes.fromhex("00000000") + offset.to_bytes(4, byteorder="little") + len(part).to_bytes(8, byteorder="little") + part),
                keys=[
                    AccountMeta(pubkey=holder, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                ]))
            receipts.append(http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))["result"])
            offset += len(part)

        for rcpt in receipts:
            confirm_transaction(http_client, rcpt)

    def call_partial_signed(self, input, contract, code):
        tx = {'to': solana2ether(contract), 'value': 0, 'gas': 9999999, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        instruction = from_addr + sign + msg

        storage = self.create_account_with_seed(sign[:8].hex())

        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 9)))
        trx.add(self.sol_instr_09_partial_call(storage, 0, instruction, contract, code))
        send_transaction(http_client, trx, self.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(self.sol_instr_10_continue(storage, 400, contract, code))
            result = send_transaction(http_client, trx, self.acc)["result"]

            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    return result


    def call_with_holder_account(self, input, contract, code):
        tx = {'to': solana2ether(contract), 'value': 0, 'gas': 9999999, 'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)

        holder = self.create_account_with_seed("1236")
        storage = self.create_account_with_seed(sign[:8].hex())

        self.write_transaction_to_holder_account(holder, sign, msg)

        trx = Transaction()
        trx.add(self.sol_instr_11_partial_call_from_account(holder, storage, 0, contract, code))
        send_transaction(http_client, trx, self.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(self.sol_instr_10_continue(storage, 400, contract, code))
            result = send_transaction(http_client, trx, self.acc)["result"]
            
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
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)

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
        tx = {'to': solana2ether(self.reId_caller), 'value': 0, 'gas': 9999999, 'gasPrice': 1,
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
        result = self.call_with_holder_account(input=data, contract=self.reId_caller, code=self.reId_caller_code)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 4) # TODO: why not 3?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)

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
        if http_client.get_balance(self.reId_create_receiver_code_account, commitment='recent')['result']['value'] == 0:
            trx = Transaction()
            trx.add(
                createAccountWithSeed(self.acc.public_key(),
                                      self.acc.public_key(),
                                      self.reId_create_receiver_seed,
                                      10 ** 9,
                                      4096 + 4 * 1024,
                                      PublicKey(evm_loader_id)))
            res = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]

        if http_client.get_balance(self.reId_create_receiver, commitment='recent')['result']['value'] == 0:
            trx = Transaction()
            trx.add(
                self.loader.createEtherAccountTrx(self.reId_create_receiver_eth, self.reId_create_receiver_code_account)[0]
            )
            res = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]

        func_name = abi.function_signature_to_4byte_selector('creator()')
        result = self.call_partial_signed(input=func_name, contract=self.reId_create_caller, code=self.reId_create_caller_code)

        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3) # TODO: why not 2?
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)

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
        self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)

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

if __name__ == '__main__':
    unittest.main()
