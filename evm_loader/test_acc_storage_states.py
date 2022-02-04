import unittest
from base58 import b58decode
from solana_utils import *
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))

class storage_states(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_acc_storage_states.py setUpClass")

        wallet = OperatorAccount(operator1_keypair_path())

        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()
        cls.acc_token = get_associated_token_address(PublicKey(cls.acc.public_key()), ETH_TOKEN_MINT_ID)

        if getBalance(wallet.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])


        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create.caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")

        # SplToken(solana_url).transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID))

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        (cls.reId, cls.reId_eth, cls.re_code) = cls.loader.deployChecked(
            CONTRACTS_DIR + "rw_lock.binary", cls.caller, cls.caller_ether)
        print('contract', cls.reId)
        print('contract_eth', cls.reId_eth.hex())
        print('contract_code', cls.re_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')


    def sol_instr_19_partial_call(self, storage_account, step_count, evm_instruction, writable_code, acc, caller,
                                  add_meta=[]):
        neon_evm_instr_19_partial_call = create_neon_evm_instr_19_partial_call(
            self.loader.loader_id,
            caller,
            acc.public_key(),
            storage_account,
            self.reId,
            self.re_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            step_count,
            evm_instruction,
            writable_code,
            add_meta,
        )
        print('neon_evm_instr_19_partial_call:', neon_evm_instr_19_partial_call)
        return neon_evm_instr_19_partial_call

    def sol_instr_13_partial_call_or_continue(self, storage_account, step_count, evm_instruction, writable_code, acc, caller,
                                  add_meta=[]):
        neon_evm_instr_13_partial_call_or_continue = create_neon_evm_instr_13_partial_call_or_continue(
            self.loader.loader_id,
            caller,
            acc.public_key(),
            storage_account,
            self.reId,
            self.re_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            step_count,
            evm_instruction,
            writable_code,
            add_meta,
        )
        print('neon_evm_instr_13_partial_call_or_continue:', neon_evm_instr_13_partial_call_or_continue)
        return neon_evm_instr_13_partial_call_or_continue

    def sol_instr_20_continue(self, storage_account, step_count, writable_code, acc, caller, add_meta=[]):
        neon_evm_instr_20_continue = create_neon_evm_instr_20_continue(
            self.loader.loader_id,
            caller,
            acc.public_key(),
            storage_account,
            self.reId,
            self.re_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            step_count,
            writable_code,
            add_meta,
        )
        print('neon_evm_instr_20_continue:', neon_evm_instr_20_continue)
        return neon_evm_instr_20_continue

    def neon_emv_instr_cancel_21(self, acc, caller, storage, nonce):
        neon_evm_instr_21_cancel = create_neon_evm_instr_21_cancel(
            self.loader.loader_id,
            caller,
            acc.public_key(),
            storage,
            self.reId,
            self.re_code,
            nonce
        )
        print('neon_evm_instr_21_cancel:', neon_evm_instr_21_cancel)
        return neon_evm_instr_21_cancel

    def call_begin(self, storage, steps, msg, instruction, writable_code, acc, caller, add_meta=[]):
        print("Begin")
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 13)))
        trx.add(self.sol_instr_19_partial_call(storage, steps, instruction, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def call_begin_0D(self, storage, steps, msg, instruction, writable_code, acc, caller, add_meta=[]):
        print("Begin, combined mode")
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 13)))
        trx.add(self.sol_instr_13_partial_call_or_continue(storage, steps, instruction, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def call_continue(self, storage, steps, writable_code, acc, caller, add_meta=[]):
        print("Continue")
        trx = Transaction()
        trx.add(self.sol_instr_20_continue(storage, steps, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def call_continue_0D(self, storage, steps, msg, instruction, writable_code, acc, caller, add_meta=[]):
        print("Continue, combined mode")
        trx = Transaction()
        trx.add(self.sol_instr_13_partial_call_or_continue(storage, steps, instruction, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def get_call_parameters(self, input, acc, caller, caller_ether, nonce_increment=0):
        nonce = getTransactionCount(client, caller) + nonce_increment
        tx = {'to': self.reId_eth, 'value': 0, 'gas': 99999999, 'gasPrice': 0,
              'nonce': nonce, 'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, acc.secret_key())
        assert (from_addr == caller_ether)
        return (from_addr, sign, msg, nonce)

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def create_storage_account(self, seed, acc):
        storage = PublicKey(
            sha256(bytes(acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10 ** 9, 128 * 1024,
                                          PublicKey(evm_loader_id)))
            send_transaction(client, trx, acc)

        return storage

    def check_continue_result(self, result):
        # if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
        self.assertEqual(data[0], 6)


    # two iterative transactions without combined mode is performed one by one
    def test_not_combined(self):
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        input2 = (func_name + bytes.fromhex("%064x" % 0x2) + bytes.fromhex("%064x" % 0x2))


        (from_addr1, sign1, msg1, _) = self.get_call_parameters(input1, self.acc, self.caller, self.caller_ether)
        (from_addr2, sign2, msg2, _) = self.get_call_parameters(input2, self.acc, self.caller, self.caller_ether, nonce_increment=1)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        # one storage account for both transactions
        storage = self.create_storage_account(sign1[:8].hex(), self.acc)

        result1 = self.call_begin(storage, 10, msg1, instruction1, False, self.acc, self.caller)
        result1 = self.call_continue(storage, 10, False, self.acc, self.caller)
        result1 = self.call_continue(storage, 1000, False, self.acc, self.caller)

        result2 = self.call_begin(storage, 10, msg2, instruction2, False, self.acc, self.caller)
        result2 = self.call_continue(storage, 10, False, self.acc, self.caller)
        result2 = self.call_continue(storage, 1000, False, self.acc, self.caller)

        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        for (result, sum) in ([ (result1["result"], 2), (result2["result"],4) ]):
            print('result:', result)
            self.assertEqual(result['meta']['err'], None)
            self.assertEqual(len(result['meta']['innerInstructions']), 1)
            # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
            self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[:1], b'\x06')  # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            self.assertEqual(data[10:], bytes().fromhex("%064x" % sum))


   # two iterative transactions in combined mode is performed one by one
    def test_combined(self):
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        input2 = (func_name + bytes.fromhex("%064x" % 0x2) + bytes.fromhex("%064x" % 0x2))


        (from_addr1, sign1, msg1, _) = self.get_call_parameters(input1, self.acc, self.caller, self.caller_ether)
        (from_addr2, sign2, msg2, _) = self.get_call_parameters(input2, self.acc, self.caller, self.caller_ether, nonce_increment=1)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        # one storage account for both transactions
        storage = self.create_storage_account(sign1[:8].hex(), self.acc)

        result1 = self.call_begin_0D(storage, 10, msg1, instruction1, False, self.acc, self.caller)
        result1 = self.call_continue_0D(storage, 10, msg1, instruction1, False, self.acc, self.caller)
        result1 = self.call_continue_0D(storage, 1000, msg1, instruction1, False, self.acc, self.caller)

        # checking, that exception contains Cutom error 0x4 (StorageAccountFinalized)
        try:
            self.call_continue_0D(storage, 1000, msg1, instruction1, False, self.acc, self.caller)
        except Exception as err:
            if str(err).startswith(
                    "Transaction simulation failed: Error processing Instruction 0: custom program error: 0x4"):
                print ("Exception was expected, OK")
                pass
            else:
                raise

        # next transaction
        result2 = self.call_begin_0D(storage, 10, msg2, instruction2, False, self.acc, self.caller)
        result2 = self.call_continue_0D(storage, 10, msg2, instruction2, False, self.acc, self.caller)
        result2 = self.call_continue_0D(storage, 1000, msg2, instruction2, False, self.acc, self.caller)

        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        for (result, sum) in ([ (result1["result"], 2), (result2["result"],4) ]):
            print('result:', result)
            self.assertEqual(result['meta']['err'], None)
            self.assertEqual(len(result['meta']['innerInstructions']), 1)
            # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
            self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[:1], b'\x06')  # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            self.assertEqual(data[10:], bytes().fromhex("%064x" % sum))


   # four iterative transactions: not combined, combined, not combibed, combined
    def test_combined_and_not_combined(self):
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        input2 = (func_name + bytes.fromhex("%064x" % 0x2) + bytes.fromhex("%064x" % 0x2))
        input3 = (func_name + bytes.fromhex("%064x" % 0x3) + bytes.fromhex("%064x" % 0x3))
        input4 = (func_name + bytes.fromhex("%064x" % 0x4) + bytes.fromhex("%064x" % 0x4))


        (from_addr1, sign1, msg1, _) = self.get_call_parameters(input1, self.acc, self.caller, self.caller_ether)
        (from_addr2, sign2, msg2, _) = self.get_call_parameters(input2, self.acc, self.caller, self.caller_ether, nonce_increment=1)
        (from_addr3, sign3, msg3, _) = self.get_call_parameters(input3, self.acc, self.caller, self.caller_ether, nonce_increment=2)
        (from_addr4, sign4, msg4, _) = self.get_call_parameters(input4, self.acc, self.caller, self.caller_ether, nonce_increment=3)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2
        instruction3 = from_addr3 + sign3 + msg3
        instruction4 = from_addr4 + sign4 + msg4

        # one storage account for both transactions
        storage = self.create_storage_account(sign1[:8].hex(), self.acc)

        print("first transaction:")
        result1 = self.call_begin(storage, 10, msg1, instruction1, False, self.acc, self.caller)
        result1 = self.call_continue(storage, 10, False, self.acc, self.caller)
        result1 = self.call_continue(storage, 1000, False, self.acc, self.caller)

        print("second transaction:")
        result2 = self.call_begin_0D(storage, 10, msg2, instruction2, False, self.acc, self.caller)
        result2 = self.call_continue_0D(storage, 10, msg2, instruction2, False, self.acc, self.caller)
        result2 = self.call_continue_0D(storage, 1000, msg2, instruction2, False, self.acc, self.caller)

        print("third transaction:")
        result3 = self.call_begin(storage, 10, msg3, instruction3, False, self.acc, self.caller)
        result3 = self.call_continue(storage, 10, False, self.acc, self.caller)
        result3 = self.call_continue(storage, 1000, False, self.acc, self.caller)

        print("fourth transaction:")
        result4 = self.call_begin_0D(storage, 10, msg4, instruction4, False, self.acc, self.caller)
        result4 = self.call_continue_0D(storage, 10, msg4, instruction4, False, self.acc, self.caller)
        result4 = self.call_continue_0D(storage, 1000, msg4, instruction4, False, self.acc, self.caller)


        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        for (result, sum) in ([ (result1["result"], 2), (result2["result"],4), (result3["result"],6), (result4["result"],8) ]):
            print('result:', result)
            self.assertEqual(result['meta']['err'], None)
            self.assertEqual(len(result['meta']['innerInstructions']), 1)
            # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
            self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[:1], b'\x06')  # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            self.assertEqual(data[10:], bytes().fromhex("%064x" % sum))
