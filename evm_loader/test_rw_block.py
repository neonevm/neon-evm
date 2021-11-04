from solana.rpc.api import SendTransactionError
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


def emulate(caller, contract, data, value):
    cmd = "{} {} {} {}".format(caller, contract, data, value)
    output = neon_cli().emulate(evm_loader_id, cmd)
    result = json.loads(output)
    if result["exit_status"] != "succeed":
        raise Exception("evm emulator error ", result)
    return result

def create_account_with_seed(client, funding, base, seed, storage_size):
    account = accountWithSeed(base.public_key(), seed, PublicKey(evm_loader_id))

    if client.get_balance(account, commitment=Confirmed)['result']['value'] == 0:
        minimum_balance = client.get_minimum_balance_for_rent_exemption(storage_size, commitment=Confirmed)["result"]
        print("Minimum balance required for account {}".format(minimum_balance))

        trx = TransactionWithComputeBudget()
        trx.add(createAccountWithSeed(funding.public_key(), base.public_key(), seed, minimum_balance, storage_size, PublicKey(evm_loader_id)))
        send_transaction(client, trx, funding)

    return account


class RW_Locking_Test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_event.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet1 = OperatorAccount(operator1_keypair_path())

        cls.loader = EvmLoader(wallet1, evm_loader_id)
        cls.acc1 = wallet1.get_acc()

        if getBalance(wallet1.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet1.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        # cls.token.transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(wallet1.get_acc().public_key(), ETH_TOKEN_MINT_ID))
        # cls.token.mint(ETH_TOKEN_MINT_ID, get_associated_token_address(PublicKey(wallet1.get_acc().public_key()), ETH_TOKEN_MINT_ID), 10000)


        # Create ethereum account for user account
        cls.caller1_ether = eth_keys.PrivateKey(cls.acc1.secret_key()).public_key.to_canonical_address()
        (cls.caller1, cls.caller1_nonce) = cls.loader.ether2program(cls.caller1_ether)
        cls.caller1_token = get_associated_token_address(PublicKey(cls.caller1), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller1) == 0:
            print("Create.caller1 account...")
            _ = cls.loader.createEtherAccount(cls.caller1_ether)
            print("Done\n")
        cls.token.transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(PublicKey(cls.caller1), ETH_TOKEN_MINT_ID))

        print('Account1:', cls.acc1.public_key(), bytes(cls.acc1.public_key()).hex())
        print("Caller1:", cls.caller1_ether.hex(), cls.caller1_nonce, "->", cls.caller1,
              "({})".format(bytes(PublicKey(cls.caller1)).hex()))


        (cls.reId, cls.reId_eth, cls.re_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"rw_lock.binary", cls.caller1, cls.caller1_ether)
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())
        print ('contract_code', cls.re_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

        # other wallet
        wallet2 = OperatorAccount(operator2_keypair_path())
        cls.acc2 = wallet2.get_acc()

        if getBalance(wallet2.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet2.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        # cls.wallet2_token = cls.token.create_token_account(ETH_TOKEN_MINT_ID, owner=wallet2.get_path())
        # cls.token.mint(ETH_TOKEN_MINT_ID, get_associated_token_address(PublicKey(wallet2.get_acc().public_key()), ETH_TOKEN_MINT_ID), 10000)


        cls.caller2_ether = eth_keys.PrivateKey(cls.acc2.secret_key()).public_key.to_canonical_address()
        (cls.caller2, cls.caller2_nonce) = cls.loader.ether2program(cls.caller2_ether)

        if getBalance(cls.caller2) == 0:
            print("Create caller2 account...")
            _ = cls.loader.createEtherAccount(cls.caller2_ether)
            print("Done\n")

        cls.token.transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(PublicKey(cls.caller2), ETH_TOKEN_MINT_ID))

        print('Account2:', cls.acc2.public_key(), bytes(cls.acc2.public_key()).hex())
        print("Caller2:", cls.caller2_ether.hex(), cls.caller2_nonce, "->", cls.caller2,
              "({})".format(bytes(PublicKey(cls.caller2)).hex()))


    def sol_instr_19_partial_call(self, storage_account, step_count, evm_instruction, writable_code, acc, caller, add_meta=[]):
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

    def call_begin(self, storage, steps, msg, instruction,  writable_code, acc, caller, add_meta=[]):
        print("Begin")
        trx = TransactionWithComputeBudget()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 13)))
        trx.add(self.sol_instr_19_partial_call(storage, steps, instruction, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def call_continue(self, storage, steps, writable_code, acc, caller, add_meta=[]):
        print("Continue")
        trx = TransactionWithComputeBudget()
        trx.add(self.sol_instr_20_continue(storage, steps, writable_code, acc, caller, add_meta))
        return send_transaction(client, trx, acc)

    def get_call_parameters(self, input, acc, caller, caller_ether):
        nonce = getTransactionCount(client, caller)
        tx = {'to': self.reId_eth, 'value': 0, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': nonce, 'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, acc.secret_key())
        assert (from_addr == caller_ether)
        return (from_addr, sign, msg, nonce)

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def create_storage_account(self, seed, acc):
        storage = PublicKey(sha256(bytes(acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = TransactionWithComputeBudget()
            trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, acc)

        return storage

    def check_continue_result(self, result):
        if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[0], 6)

    # the contract account is locked by the read-only lock
    # two transactions of the one contract are executed by two callers
    # @unittest.skip("a.i.")
    def test_01_caseReadOlnyBlocking(self):
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr1, sign1, msg1, _) = self.get_call_parameters(input, self.acc1, self.caller1, self.caller1_ether)
        (from_addr2, sign2, msg2, _) = self.get_call_parameters(input, self.acc2, self.caller2, self.caller2_ether)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        storage1 = self.create_storage_account(sign1[:8].hex(), self.acc1)
        storage2 = self.create_storage_account(sign2[1:9].hex(), self.acc2)

        result = self.call_begin(storage1, 10, msg1, instruction1, False, self.acc1, self.caller1)
        result = self.call_begin(storage2, 10, msg2, instruction2, False, self.acc2, self.caller2)
        result = self.call_continue(storage1, 10, False, self.acc1, self.caller1)
        result = self.call_continue(storage2, 10, False, self.acc2, self.caller2)
        result1 = self.call_continue(storage1, 1000, False, self.acc1, self.caller1)
        result2 = self.call_continue(storage2, 1000, False, self.acc2, self.caller2)

        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        for result in ([result1["result"], result2["result"]]):
            print('result:', result)
            self.assertEqual(result['meta']['err'], None)
            self.assertEqual(len(result['meta']['innerInstructions']), 1)
            # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
            self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            self.assertEqual(int().from_bytes(data[2:10], 'little'), 21663) # used_gas
            self.assertEqual(data[10:], bytes().fromhex("%064x" % 0x2))

    # The first transaaction set lock on write to  contract account
    # The second transaction try to set lock on write and  => the error occurs.
    # Then lock removed by Cancel operation
    # @unittest.skip("a.i.")
    def test_02_caseWriteBlocking(self):
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1))

        (from_addr1, sign1, msg1, nonce1) = self.get_call_parameters(input, self.acc1, self.caller1, self.caller1_ether)
        (from_addr2, sign2, msg2, nonce2) = self.get_call_parameters(input, self.acc2, self.caller2, self.caller2_ether)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        storage1 = self.create_storage_account(sign1[:8].hex(), self.acc1)
        storage2 = self.create_storage_account(sign2[1:9].hex(), self.acc2)

        result = self.call_begin(storage1, 10, msg1, instruction1, True, self.acc1, self.caller1)

        try:
            result = self.call_begin(storage2, 10, msg2, instruction2, True, self.acc2, self.caller2)
        except SendTransactionError as err:
            print("Ok")

            # removing the rw-lock
            trx = TransactionWithComputeBudget().add(self.neon_emv_instr_cancel_21(self.acc1, self.caller1, storage1, nonce1))
            response = send_transaction(client, trx, self.acc1)
            return

        # removing the rw-lock
        trx = TransactionWithComputeBudget().add(self.neon_emv_instr_cancel_21(self.acc1, self.caller1, storage1, nonce1))
        response = send_transaction(client, trx, self.acc1)
        raise("error, account was not block")


    def check_writable(self, res, contract, writable_expected):
        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == contract:
                self.assertEqual(info["writable"], writable_expected)
                return
        raise("contract_eth not found in  the emulator output, ", self.reId_eth)

    def test_03_writable_flag_from_emulator(self):
        # 1. "writable" must be False. Storage is not changed
        print("reId_code", self.re_code)

        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        res = emulate(self.caller1_ether.hex(), self.reId_eth.hex(), input.hex(), "" )
        self.check_writable(res, self.reId_eth, False)
        print(res)

        # 2. "writable" must be True. Storage is changed
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        input = (func_name + bytes.fromhex("%064x" % 0x1))
        res = emulate(self.caller1_ether.hex(), self.reId_eth.hex(), input.hex(), "" )
        self.check_writable(res, self.reId_eth, True)
        print(res)

        # 3. "writable" must be True. Contract nonce is changed
        func_name = abi.function_signature_to_4byte_selector('deploy_contract()')
        res = emulate(self.caller1_ether.hex(), self.reId_eth.hex(), func_name.hex(), "" )
        new_contract_eth = bytes.fromhex(res["result"][-40:])
        self.check_writable(res, self.reId_eth, True)
        print(res)

        # apply last tansaction (the method deploys the contract)
        meta=None
        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == new_contract_eth:
                seed = b58encode(ACCOUNT_SEED_VERSION + new_contract_eth).decode('utf8')
                new_contract_code = accountWithSeed(self.acc1.public_key(), seed, PublicKey(evm_loader_id))
                create_account_with_seed(client, self.acc1, self.acc1, seed, info["code_size"])

                (trx, _) = self.loader.createEtherAccountTrx(new_contract_eth, new_contract_code)
                send_transaction(client, trx, self.acc1)

                meta = [
                    AccountMeta(pubkey=PublicKey(info["account"]), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=get_associated_token_address(PublicKey(info["account"]), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=PublicKey(new_contract_code), is_signer=False, is_writable=True),
                       ]
                print("new_contract_code", new_contract_code)

        self.assertNotEqual(meta, None)

        (from_addr, sign, msg, _) = self.get_call_parameters(func_name, self.acc1, self.caller1, self.caller1_ether)
        instruction = from_addr + sign + msg
        storage = self.create_storage_account(sign[:8].hex(), self.acc1)

        result = self.call_begin(storage, 10, msg, instruction, False, self.acc1, self.caller1, meta)
        result = self.call_continue(storage, 1000, True, self.acc1, self.caller1, meta)
        self.check_continue_result(result["result"])


        # 4. "writable" must be False. Contract calls the method of other contract. Contract nonce is not changed
        func_name = abi.function_signature_to_4byte_selector('call_hello_world()')
        res = emulate(self.caller1_ether.hex(), new_contract_eth.hex(), func_name.hex(), "" )
        print(res)
        self.check_writable(res, new_contract_eth, False)


    #  the test must be run last, because it changes contract code account
    #  resizing is blocked  by locking of the account in other transaction.
    def test_04_resizing_with_account_lock(self):

        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1)) # update storage without account resizing
        input2 = (func_name + bytes.fromhex("%064x" % 0x20)) # update storage with account resizing

        (from_addr1, sign1,  msg1, _) = self.get_call_parameters(input1, self.acc1, self.caller1, self.caller1_ether)
        instruction1 = from_addr1 + sign1 + msg1
        storage1 = self.create_storage_account(sign1[:8].hex(), self.acc1)

        # start first transaction
        self.call_begin(storage1, 10, msg1, instruction1, True, self.acc1, self.caller1)

        #emulate second transaction
        res = emulate(self.caller2_ether.hex(), self.reId_eth.hex(), input2.hex(), "" )
        print(res)
        resize_instr = None
        code_account_new = None
        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == self.reId_eth:
                self.assertEqual(info["writable"], True)
                self.assertEqual(info["code_size"] > info["code_size_current"], True)

                code_size = info["code_size"] + 2048
                seed_bin = b58encode(ACCOUNT_SEED_VERSION + os.urandom(20))
                seed = seed_bin.decode('utf8')
                code_account_new = accountWithSeed(self.acc2.public_key(), seed, PublicKey(evm_loader_id))

                print("creating new code_account with increased size %s", code_account_new)
                create_account_with_seed(client, self.acc2, self.acc2, seed, code_size);

                resize_instr = TransactionInstruction(
                    keys=[
                        AccountMeta(pubkey=PublicKey(info["account"]), is_signer=False, is_writable=True),
                        AccountMeta(pubkey=info["contract"], is_signer=False, is_writable=True),
                        AccountMeta(pubkey=code_account_new, is_signer=False, is_writable=True),
                        AccountMeta(pubkey=self.acc2.public_key(), is_signer=True, is_writable=False)
                    ],
                    program_id=evm_loader_id,
                    data=bytearray.fromhex("11") + bytes(seed_bin)  # 17- ResizeStorageAccount
                )
                break

        self.assertIsNotNone(resize_instr)
        # send resizing transaction
        send_transaction(client, TransactionWithComputeBudget().add(resize_instr), self.acc2)
        # get info about resizing account
        info = getAccountData(client, self.reId, ACCOUNT_INFO_LAYOUT.sizeof())
        info_data = AccountInfo.frombytes(info)

        # resizing must not be completed due to locking contract account.
        self.assertEqual(info_data.code_account, PublicKey(self.re_code))

        # finish first transaction for unlocking accounts
        self.call_continue(storage1, 1000, True, self.acc1, self.caller1)

        # before resizing the old code account must have some balance
        self.assertNotEqual(getBalance(self.re_code), 0)

        # try next attempt to resize storage account and check it
        send_transaction(client, TransactionWithComputeBudget().add(resize_instr), self.acc2)
        info = getAccountData(client, self.reId, ACCOUNT_INFO_LAYOUT.sizeof())
        info_data = AccountInfo.frombytes(info)

        # resizing must be completed => code_account must be updated
        self.assertNotEqual(info_data.code_account, self.re_code)

        # afrer resizing the old code account must have 0 SOL
        self.assertEqual(getBalance(self.re_code), 0)

if __name__ == '__main__':
    unittest.main()
