from solana.publickey import PublicKey
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from spl.token.instructions import get_associated_token_address
from spl.token.constants import TOKEN_PROGRAM_ID, ACCOUNT_LEN
import unittest
from eth_utils import abi
from base58 import b58decode
import re

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, JsonEncoder
from solana_utils import *

CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
holder_id = 0

class PrecompilesTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_block_hashses.py setUpClass")

        cls.token = SplToken(solana_url)
        wallet = OperatorAccount(operator1_keypair_path())
        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        cls.caller_token = get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            cls.token.transfer(ETH_TOKEN_MINT_ID, 201, get_associated_token_address(PublicKey(cls.caller), ETH_TOKEN_MINT_ID))
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        print("deploy contract: ")
        (cls.owner_contract, cls.eth_contract, cls.contract_code) = cls.loader.deployChecked(
                CONTRACTS_DIR+'BlockHashTest.binary',
                cls.caller,
                cls.caller_ether
            )
        print("contract id: ", cls.owner_contract, cls.eth_contract)
        print("code id: ", cls.contract_code)

        collateral_pool_index = 2
        cls.collateral_pool_address = create_collateral_pool_address(collateral_pool_index)
        cls.collateral_pool_index_buf = collateral_pool_index.to_bytes(4, 'little')

    def send_transaction(self, data):
        if len(data) > 512:
            result = self.call_with_holder_account(data)
            return b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])[8+2:].hex()
        else:
            trx = self.make_transactions(data)
            result = send_transaction(client, trx, self.acc)
            self.get_measurements(result)
            result = result["result"]
            return b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])[8+2:].hex()

    def extract_measurements_from_receipt(self, receipt):
        log_messages = receipt['result']['meta']['logMessages']
        transaction = receipt['result']['transaction']
        accounts = transaction['message']['accountKeys']
        instructions = []
        for instr in transaction['message']['instructions']:
            program = accounts[instr['programIdIndex']]
            instructions.append({
                'accs': [accounts[acc] for acc in instr['accounts']],
                'program': accounts[instr['programIdIndex']],
                'data': b58decode(instr['data']).hex()
            })

        pattern = re.compile('Program ([0-9A-Za-z]+) (.*)')
        messages = []
        for log in log_messages:
            res = pattern.match(log)
            if res:
                (program, reason) = res.groups()
                if reason == 'invoke [1]': messages.append({'program':program,'logs':[]})
            messages[-1]['logs'].append(log)

        for instr in instructions:
            if instr['program'] in ('KeccakSecp256k11111111111111111111111111111',): continue
            if messages[0]['program'] != instr['program']:
                raise Exception('Invalid program in log messages: expect %s, actual %s' % (messages[0]['program'], instr['program']))
            instr['logs'] = messages.pop(0)['logs']
            exit_result = re.match(r'Program %s (success)'%instr['program'], instr['logs'][-1])
            if not exit_result: raise Exception("Can't get exit result")
            instr['result'] = exit_result.group(1)

            if instr['program'] == evm_loader_id:
                memory_result = re.match(r'Program log: Total memory occupied: ([0-9]+)', instr['logs'][-3])
                instruction_result = re.match(r'Program %s consumed ([0-9]+) of ([0-9]+) compute units'%instr['program'], instr['logs'][-2])
                if not (memory_result and instruction_result):
                    raise Exception("Can't parse measurements for evm_loader")
                instr['measurements'] = {
                        'instructions': instruction_result.group(1),
                        'memory': memory_result.group(1)
                    }

        result = []
        for instr in instructions:
            if instr['program'] == evm_loader_id:
                result.append({
                        'program':instr['program'],
                        'measurements':instr['measurements'],
                        'result':instr['result'],
                        'data':instr['data']
                    })
        return result

    def get_measurements(self, result):
        measurements = self.extract_measurements_from_receipt(result)
        for m in measurements: print(json.dumps(m))

    def make_transactions(self, call_data):
        eth_tx = {
            'to': self.eth_contract,
            'value': 0,
            'gas': 9999999,
            'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller),
            'data': call_data,
            'chainId': 111
        }

        (_from_addr, sign, msg) = make_instruction_data_from_tx(eth_tx, self.acc.secret_key())
        trx_data = self.caller_ether + sign + msg
        keccak_instruction = make_keccak_instruction_data(1, len(msg), 5)
        
        solana_trx = Transaction().add(
                self.sol_instr_keccak(keccak_instruction) 
            ).add( 
                self.sol_instr_call(trx_data) 
            )

        return solana_trx

    def sol_instr_keccak(self, keccak_instruction):
        return  TransactionInstruction(program_id="KeccakSecp256k11111111111111111111111111111", data=keccak_instruction, keys=[
                    AccountMeta(pubkey=self.caller, is_signer=False, is_writable=False),
                ])

    def sol_instr_call(self, trx_data):
        neon_evm_instr_05_single = create_neon_evm_instr_05_single(
            self.loader.loader_id,
            self.caller,
            self.acc.public_key(),
            self.owner_contract,
            self.contract_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            trx_data,
            add_meta=[AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False),]
        )
        print('neon_evm_instr_05_single:', neon_evm_instr_05_single)
        return neon_evm_instr_05_single

    def sol_instr_22_partial_call_from_account(self, holder_account, storage_account, step_count):
        neon_evm_instr_22_begin = create_neon_evm_instr_22_begin(
            self.loader.loader_id,
            self.caller,
            self.acc.public_key(),
            storage_account,
            holder_account,
            self.owner_contract,
            self.contract_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            step_count,
            add_meta=[AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False),]
        )
        print('neon_evm_instr_22_begin:', neon_evm_instr_22_begin)
        return neon_evm_instr_22_begin

    def sol_instr_20_continue(self, storage_account, step_count):
        neon_evm_instr_20_continue = create_neon_evm_instr_20_continue(
            self.loader.loader_id,
            self.caller,
            self.acc.public_key(),
            storage_account,
            self.owner_contract,
            self.contract_code,
            self.collateral_pool_index_buf,
            self.collateral_pool_address,
            step_count,
            add_meta=[AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False),]
        )
        print('neon_evm_instr_20_continue:', neon_evm_instr_20_continue)
        return neon_evm_instr_20_continue

    def create_account_with_seed(self, seed):
        storage = accountWithSeed(self.acc.public_key(), seed, PublicKey(evm_loader_id))

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10**9, 128*1024, PublicKey(evm_loader_id)))
            client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="confirmed"))

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
                data=(bytes.fromhex('12') + holder_id.to_bytes(8, byteorder="little") + offset.to_bytes(4, byteorder="little") + len(part).to_bytes(8, byteorder="little") + part),
                keys=[
                    AccountMeta(pubkey=holder, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                ]))
            receipts.append(client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))["result"])
            offset += len(part)

        for rcpt in receipts:
            confirm_transaction(client, rcpt)


    def call_with_holder_account(self, input):
        tx = {'to': self.eth_contract, 'value': 0, 'gas': 9999999, 'gasPrice': 1_000_000_000,
            'nonce': getTransactionCount(client, self.caller), 'data': input, 'chainId': 111}

        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)

        holder_id_bytes = holder_id.to_bytes((holder_id.bit_length() + 7) // 8, 'big')
        holder_seed = keccak_256(b'holder'+holder_id_bytes).hexdigest()[:32]
        holder = self.create_account_with_seed(holder_seed)
        storage = self.create_account_with_seed(sign[:8].hex())

        self.write_transaction_to_holder_account(holder, sign, msg)

        trx = Transaction()
        trx.add(self.sol_instr_22_partial_call_from_account(holder, storage, 0))
        send_transaction(client, trx, self.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(self.sol_instr_20_continue(storage, 400))
            result = send_transaction(client, trx, self.acc)

            self.get_measurements(result)
            result = result["result"]

            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    return result

    def make_getCurrentValues(self):
        return abi.function_signature_to_4byte_selector('getCurrentValues()')

    def make_getValues(self, number: int):
        return abi.function_signature_to_4byte_selector('getValues(uint number)')\
                + bytes.fromhex("%062x" % number)

    def get_blocks_from_solana(self):
        slot_hash = {}
        current_slot = client.get_slot()["result"]
        for slot in range(current_slot):
            hash_val = base58.b58decode(client.get_confirmed_block(slot)['result']['blockhash']).hex()
            slot_hash[int(slot)] = hash_val
        return slot_hash

    def test_01_block_hashes(self):
        print("test_01_block_hashes")
        solana_result = self.get_blocks_from_solana()
        for sol_slot, sol_hash in solana_result.items():
            result = self.send_transaction(self.make_getValues(sol_slot))
            print(f"sol_slot: {sol_slot} sol_hash: {sol_hash} result: {result}")

    def test_02_block_hashes(self):
        print("test_02_block_hashes")
        result = self.send_transaction(self.make_getCurrentValues())
        print(f"result: {result}")


if __name__ == '__main__':
    unittest.main()
