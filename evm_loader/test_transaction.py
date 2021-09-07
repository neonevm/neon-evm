import unittest

import solana
from base58 import b58decode

from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
evm_loader_id = os.environ.get("EVM_LOADER")
INVALID_NONCE = 'Invalid Ethereum transaction nonce'
INCORRECT_PROGRAM_ID = 'Incorrect Program Id'


class EvmLoaderTestsNewAccount(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_transaction.py setUpClass")

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
            cls.token.transfer(ETH_TOKEN_MINT_ID, 2000, cls.caller_token)
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        program_and_code = cls.loader.deployChecked(
            CONTRACTS_DIR + 'helloWorld.binary',
            cls.caller,
            cls.caller_ether
        )
        cls.owner_contract = program_and_code[0]
        cls.contract_code = program_and_code[2]

        print("contract id: ", cls.owner_contract, solana2ether(cls.owner_contract).hex())
        print("code id: ", cls.contract_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

    def create_storage_account(self, seed):
        storage = PublicKey(
            sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10 ** 9, 128 * 1024,
                                          PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)

        return storage

    def get_tx(self):
        return {
            'to': solana2ether(self.owner_contract),
            'value': 0,
            'gas': 9999999,
            'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller),
            'data': '3917b3df',
            'chainId': 111
        }

    def get_keccak_instruction_and_trx_data(self, data_start):
        tx = self.get_tx()
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        keccak_instruction_data = make_keccak_instruction_data(1, len(msg), data_start)
        trx_data = self.caller_ether + sign + msg

        keccak_instruction = TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111",
                                                    data=keccak_instruction_data,
                                                    keys=[AccountMeta(pubkey=self.caller, is_signer=False, is_writable=False)]
                                                    )
        return keccak_instruction, trx_data, sign

    def get_account_metas_for_instr_05(self):
        return [
            # System instructions account:
            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            # Operator address:
            AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=True),
            # Collateral pool address:
            AccountMeta(pubkey=self.collateral_pool_address, is_signer=False, is_writable=True),
            # Operator ETH address (stub for now):
            AccountMeta(pubkey=get_associated_token_address(self.acc.public_key(), ETH_TOKEN_MINT_ID),
                        is_signer=False, is_writable=True),
            # User ETH address (stub for now):
            AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID),
                        is_signer=False, is_writable=True),
            # System program account:
            AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

            AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(self.owner_contract), ETH_TOKEN_MINT_ID),
                        is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.contract_code, is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller), ETH_TOKEN_MINT_ID),
                        is_signer=False, is_writable=True),

            AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
            AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
        ]

    def get_account_metas_for_instr_0D(self, storage):
        return [AccountMeta(pubkey=storage, is_signer=False, is_writable=True)] + self.get_account_metas_for_instr_05()

    def neon_emv_instr_05(self, trx_data):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("05") + self.collateral_pool_index_buf + trx_data,
            keys=self.get_account_metas_for_instr_05()
        )

    def neon_emv_instr_0D(self, step_count, trx_data, storage):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("0D") + self.collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little') + trx_data,
            keys=self.get_account_metas_for_instr_0D(storage)
        )

    # @unittest.skip("a.i.")
    def test_01_success_tx_send(self):
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(5)
        trx = Transaction() \
            .add(keccak_instruction) \
            .add(self.neon_emv_instr_05(trx_data))

        response = send_transaction(client, trx, self.acc)
        print('response:', response)

    # @unittest.skip("a.i.")
    def test_02_success_tx_send_iteratively_in_3_solana_transactions_sequentially(self):
        step_count = 100
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(13)
        storage = self.create_storage_account(sign[:8].hex())
        neon_emv_instr_0d = self.neon_emv_instr_0D(step_count, trx_data, storage)

        trx = Transaction() \
            .add(keccak_instruction) \
            .add(neon_emv_instr_0d)

        response = send_transaction(client, trx, self.acc)
        print('response_1:', response)
        response = send_transaction(client, trx, self.acc)
        print('response_2:', response)
        response = send_transaction(client, trx, self.acc)
        print('response_3:', response)
        self.assertEqual(response['result']['meta']['err'], None)
        data = b58decode(response['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data'])
        self.assertEqual(data[0], 6)  # 6 means OnReturn,
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(int().from_bytes(data[2:10], 'little'), 24301)  # used_gas

    # @unittest.skip("a.i.")
    def test_03_failure_tx_send_iteratively_in_4_solana_transactions_sequentially(self):
        step_count = 100
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(13)
        storage = self.create_storage_account(sign[:8].hex())
        neon_emv_instr_0d = self.neon_emv_instr_0D(step_count, trx_data, storage)

        trx = Transaction() \
            .add(keccak_instruction) \
            .add(neon_emv_instr_0d)

        response = send_transaction(client, trx, self.acc)
        print('response_1:', response)
        response = send_transaction(client, trx, self.acc)
        print('response_2:', response)
        response = send_transaction(client, trx, self.acc)
        print('response_3:', response)
        try:
            send_transaction(client, trx, self.acc)
        except solana.rpc.api.SendTransactionError as err:
            print('SendTransactionError:', str(err))
            print('SendTransactionError result:', str(err.result))
            response = json.loads(str(err.result).replace('\'', '\"').replace('None', 'null'))
            print('response:', response)
            print('code:', response['code'])
            self.assertEqual(response['code'], -32002)
            print('INVALID_NONCE:', INVALID_NONCE)
            logs = response['data']['logs']
            print('logs:', logs)
            log = [s for s in logs if INVALID_NONCE in s][0]
            print(log)
            self.assertGreater(len(log), len(INVALID_NONCE))
            file_name = 'src/entrypoint.rs'
            self.assertTrue(file_name in log)
            print('the ether transaction was completed by the previous three solana transactions')
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            self.assertTrue(False)

    # @unittest.skip("a.i.")
    def test_04_success_tx_send_iteratively_by_3_instructions_in_one_transaction(self):
        step_count = 100
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(13)
        storage = self.create_storage_account(sign[:8].hex())
        neon_emv_instr_0d = self.neon_emv_instr_0D(step_count, trx_data, storage)

        trx = Transaction() \
            .add(keccak_instruction) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d)

        response = send_transaction(client, trx, self.acc)
        print('response:', response)
        self.assertEqual(response['result']['meta']['err'], None)
        data = b58decode(response['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data'])
        self.assertEqual(data[0], 6)  # 6 means OnReturn,
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        self.assertEqual(int().from_bytes(data[2:10], 'little'), 24301)  # used_gas

    # @unittest.skip("a.i.")
    def test_05_failure_tx_send_iteratively_by_4_instructions_in_one_transaction(self):
        step_count = 100
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(13)
        storage = self.create_storage_account(sign[:8].hex())
        neon_emv_instr_0d = self.neon_emv_instr_0D(step_count, trx_data, storage)

        trx = Transaction() \
            .add(keccak_instruction) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d)

        try:
            send_transaction(client, trx, self.acc)
        except solana.rpc.api.SendTransactionError as err:
            print('SendTransactionError:', str(err))
            print('SendTransactionError result:', str(err.result))
            response = json.loads(str(err.result).replace('\"', '').replace('\'', '\"').replace('None', 'null'))
            print('response:', response)
            print('code:', response['code'])
            self.assertEqual(response['code'], -32002)
            print('INCORRECT_PROGRAM_ID:', INCORRECT_PROGRAM_ID)
            logs = response['data']['logs']
            print('logs:', logs)
            log = [s for s in logs if INCORRECT_PROGRAM_ID in s][0]
            print(log)
            self.assertGreater(len(log), len(INCORRECT_PROGRAM_ID))
            file_name = 'src/transaction.rs'
            self.assertTrue(file_name in log)
            print(
                'the ether transaction was completed by the previous three instructions in the same solana transaction')
        except Exception as err:
            print('type(err):', type(err))
            print('err:', str(err))
            self.assertTrue(False)

    # @unittest.skip("a.i.")
    def test_06_failure_tx_send_iteratively_transaction_too_large(self):
        step_count = 100
        (keccak_instruction, trx_data, sign) = self.get_keccak_instruction_and_trx_data(13)
        storage = self.create_storage_account(sign[:8].hex())
        neon_emv_instr_0d = self.neon_emv_instr_0D(step_count, trx_data, storage)

        trx = Transaction() \
            .add(keccak_instruction) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d) \
            .add(neon_emv_instr_0d)

        with self.assertRaisesRegex(RuntimeError, 'transaction too large'):
            response = send_transaction(client, trx, self.acc)
            print(response)

        print('the solana transaction is too large')

    # def test_fail_on_no_signature(self):
    #     tx_1 = {
    #         'to': solana2ether(self.owner_contract),
    #         'value': 0,
    #         'gas': 1,
    #         'gasPrice': 1,
    #         'nonce': 0,
    #         'data': '3917b3df',
    #         'chainId': 1
    #     }

    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())
    #     keccak_instruction = make_keccak_instruction_data(1, len(msg), 1)

    #     (caller, caller_nonce) = self.loader.ether2programAddress(from_addr)
    #     print(" ether: " + from_addr.hex())
    #     print("solana: " + caller)
    #     print(" nonce: " + str(caller_nonce))

    #     if getBalance(caller) == 0:
    #         print("Create caller account...")
    #         caller_created = self.loader.createEtherAccount(from_addr)
    #         print("Done\n")

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("a1") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),  
    #             AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False, is_writable=False),              
    #         ]))
    #     result = client.send_transaction(trx, self.acc.get_acc())

    # def test_check_wo_checks(self):
    #     tx_1 = {
    #         'to': solana2ether(self.owner_contract),
    #         'value': 0,
    #         'gas': 0,
    #         'gasPrice': 0,
    #         'nonce': 0,
    #         'data': '3917b3df',
    #         'chainId': 1
    #     }

    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_1, self.acc.get_acc().secret_key())

    #     keccak_instruction = make_keccak_instruction_data(1, len(msg), 1)

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
    #             AccountMeta(pubkey=PublicKey("KeccakSecp256k11111111111111111111111111111"), is_signer=False, is_writable=False),
    #         ])).add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("05") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
    #         ]))
    #     result = client.send_transaction(trx, self.acc.get_acc())

    # def test_raw_tx_wo_checks(self):  
    #     tx_2 = "0xf86180808094535d33341d2ddcc6411701b1cf7634535f1e8d1680843917b3df26a013a4d8875dfc46a489c2641af798ec566d57852b94743b234517b73e239a5a22a07586d01a8a1125be7108ee6580c225a622c9baa0938f4d08abe78556c8674d58"

    #     (from_addr, sign, msg) =  make_instruction_data_from_tx(tx_2)

    #     keccak_instruction = make_keccak_instruction_data(1, len(msg), 1)

    #     trx = Transaction().add(
    #         TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
    #             AccountMeta(pubkey=PublicKey("KeccakSecp256k11111111111111111111111111111"), is_signer=False, is_writable=False),
    #         ])).add(
    #         TransactionInstruction(program_id=self.evm_loader, data=bytearray.fromhex("05") + from_addr + sign + msg, keys=[
    #             AccountMeta(pubkey=self.owner_contract, is_signer=False, is_writable=True),
    #             AccountMeta(pubkey=self.acc.get_acc().public_key(), is_signer=True, is_writable=False),
    #             AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
    #         ]))
    #     result = client.send_transaction(trx, self.acc.get_acc())


if __name__ == '__main__':
    unittest.main()
