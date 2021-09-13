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
# evm_loader_id = os.environ.get("EVM_LOADER")
evm_loader_id = "Y19LVboXPCTGAE9QqSvnvc58FRQfrRJhBfiLsanKYMM"
# ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
ETH_TOKEN_MINT_ID: PublicKey = PublicKey("HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU")

class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_event.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet = WalletAccount(wallet_path())
        # wallet = RandomAccount()
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        if getBalance(wallet.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        # cls.wallet_token = cls.token.create_token_account(ETH_TOKEN_MINT_ID, owner=wallet.get_path())
        cls.wallet_token = get_associated_token_address(PublicKey(wallet.get_acc().public_key()), ETH_TOKEN_MINT_ID)
        cls.token.mint(ETH_TOKEN_MINT_ID, cls.wallet_token, 10000)


        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create caller account...") 
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")
        cls.token.transfer(ETH_TOKEN_MINT_ID, 2000, get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID))

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        # res = solana_cli().call("config set --keypair " + wallet.get_path() + " -C config.yml")

        # res = cls.loader.deploy(CONTRACTS_DIR+"ReturnsEvents.binary", caller=cls.caller, config="config.yml")
        # (cls.reId, cls.reId_eth, cls.re_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

        (cls.reId, cls.reId_eth, cls.re_code) = cls.loader.deployChecked(
            CONTRACTS_DIR+"ReturnsEvents.binary", cls.caller, cls.caller_ether)
        print ('contract', cls.reId)
        print ('contract_eth', cls.reId_eth.hex())
        print ('contract_code', cls.re_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')



        # other wallet
        wallet_other = RandomAccount()
        cls.acc_other = wallet_other.get_acc()

        if getBalance(wallet_other.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet_other.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        cls.wallet_token = cls.token.create_token_account(ETH_TOKEN_MINT_ID, owner=wallet_other.get_path())

        cls.caller_other_ether = eth_keys.PrivateKey(cls.acc_other.secret_key()).public_key.to_canonical_address()
        (cls.caller_other, cls.caller_other_nonce) = cls.loader.ether2program(cls.caller_other_ether)

        if getBalance(cls.caller_other) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_other_ether)
            print("Done\n")

        cls.token.transfer(ETH_TOKEN_MINT_ID, 2000, get_associated_token_address(PublicKey(cls.caller_other), ETH_TOKEN_MINT_ID))

        print('Account_other:', cls.acc_other.public_key(), bytes(cls.acc_other.public_key()).hex())
        print("Caller_other:", cls.caller_other_ether.hex(), cls.caller_other_nonce, "->", cls.caller_other,
              "({})".format(bytes(PublicKey(cls.caller_other)).hex()))


    def sol_instr_09_partial_call(self, storage_account, step_count, evm_instruction, writable_code, acc, caller):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("09") + self.collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little') + evm_instruction,
            keys=[
                AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),

                # System instructions account:
                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                # Operator address:
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=True),
                # Collateral pool address:
                AccountMeta(pubkey=self.collateral_pool_address, is_signer=False, is_writable=True),
                # System program account:
                AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

                AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=writable_code),
                AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
            ])

    def sol_instr_10_continue(self, storage_account, step_count, writable_code, acc, caller):
        return TransactionInstruction(
            program_id=self.loader.loader_id,
            data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
            keys=[
                AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),

                # Operator address:
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(acc.public_key(), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # User ETH address (stub for now):
                AccountMeta(pubkey=get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                # System program account:
                AccountMeta(pubkey=PublicKey(system), is_signer=False, is_writable=False),

                AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=writable_code),
                AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
            ])


    def call_begin(self, storage, steps, msg, instruction,  writable_code, acc, caller):
        print("Begin")
        trx = Transaction()
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 13)))
        trx.add(self.sol_instr_09_partial_call(storage, steps, instruction, writable_code, acc, caller))
        return send_transaction(client, trx, acc)

    def call_continue(self, storage, steps, writable_code, acc, caller):
        print("Continue")
        trx = Transaction()
        trx.add(self.sol_instr_10_continue(storage, steps, writable_code, acc, caller))
        return send_transaction(client, trx, acc)

    def get_call_parameters(self, input, acc, caller, caller_ether):
        tx = {'to': solana2ether(self.reId), 'value': 0, 'gas': 99999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, caller), 'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, acc.secret_key())
        assert (from_addr == caller_ether)
        return (from_addr, sign, msg)

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def create_storage_account(self, seed, acc):
        storage = PublicKey(sha256(bytes(acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            send_transaction(client, trx, acc)

        return storage


    def check_continue_result(self, result):
        if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            assert (data[0] == 6)


    def test_caseReadOlnyBlocking(self):
        func_name = abi.function_signature_to_4byte_selector('addReturn(uint8,uint8)')
        input = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        (from_addr1, sign1,  msg1) = self.get_call_parameters(input, self.acc, self.caller, self.caller_ether)
        (from_addr2, sign2,  msg2) = self.get_call_parameters(input, self.acc_other, self.caller_other, self.caller_other_ether)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        storage1 = self.create_storage_account(sign1[:8].hex(), self.acc)
        storage2 = self.create_storage_account(sign2[1:9].hex(), self.acc_other)

        print("self.acc_other", self.acc_other.public_key())
        print("self.acc", self.acc.public_key())
        result = self.call_begin(storage1, 10, msg1, instruction1, False, self.acc, self.caller)
        result = self.call_begin(storage2, 10, msg2, instruction2, False, self.acc_other, self.caller_other)
        result = self.call_continue(storage1, 10, False, self.acc, self.caller)
        result = self.call_continue(storage2, 10, False, self.acc_other, self.caller_other)
        result1 = self.call_continue(storage1, 1000, False, self.acc, self.caller)
        result2 = self.call_continue(storage2, 1000, False, self.acc_other, self.caller_other)

        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        for result in ([result1["result"], result2["result"]]):
            self.assertEqual(result['meta']['err'], None)
            self.assertEqual(len(result['meta']['innerInstructions']), 1)
            self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
            self.assertEqual(result['meta']['innerInstructions'][0]['index'], 0)  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            self.assertEqual(data[:1], b'\x06') # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            self.assertEqual(int().from_bytes(data[2:10], 'little'), 21719) # used_gas
            self.assertEqual(data[10:], bytes().fromhex("%064x" % 0x2))



if __name__ == '__main__':
    unittest.main()
