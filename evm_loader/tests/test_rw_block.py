import os
import json
import pathlib
from base58 import b58decode, b58encode
from hashlib import sha256
from dataclasses import dataclass

import pytest
from solana.rpc.core import RPCException
from solana.transaction import AccountMeta, TransactionInstruction
from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.rpc.commitment import Confirmed
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi
from eth_keys import keys as eth_keys
from solana_utils import account_with_seed, EVM_LOADER, neon_cli, solana_client, TransactionWithComputeBudget, \
    create_account_with_seed, send_transaction, get_solana_balance, \
    wait_confirm_transaction, ETH_TOKEN_MINT_ID, create_treasury_pool_address, RandomAccount, \
    create_neon_evm_instr_19_partial_call, create_neon_evm_instr_20_continue, create_neon_evm_instr_21_cancel, \
    get_transaction_count, keccakprog, ACCOUNT_SEED_VERSION, get_account_data, AccountInfo, ACCOUNT_INFO_LAYOUT, \
    evm_step_cost


CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR")
if CONTRACTS_DIR is None:
    CONTRACTS_DIR = pathlib.Path(__file__).parent / "contracts"


def emulate(caller, contract, data, value):
    cmd = "{} {} {} {}".format(caller, contract, data, value)
    output = neon_cli().emulate(EVM_LOADER, cmd)
    result = json.loads(output)
    if result["exit_status"] != "succeed":
        raise Exception("evm emulator error ", result)
    return result


def create_account_with_seed_in_solana(client, funding, base, seed, storage_size):
    account = account_with_seed(base.public_key(), seed, PublicKey(EVM_LOADER))

    if client.get_balance(account, commitment=Confirmed)['result']['value'] == 0:
        minimum_balance = client.get_minimum_balance_for_rent_exemption(storage_size, commitment=Confirmed)["result"]
        print("Minimum balance required for account {}".format(minimum_balance))

        trx = TransactionWithComputeBudget()
        trx.add(
            create_account_with_seed(
                funding.public_key(), base.public_key(), seed, minimum_balance, storage_size, PublicKey(EVM_LOADER)
            )
        )
        send_transaction(client, trx, funding)

    return account


@dataclass
class Caller:
    account: Keypair
    ether: bytes
    address: str
    nonce: str
    token: PublicKey


@pytest.fixture(scope="module")
def caller1(operator_keypair, evm_loader) -> Caller:
    # Create ethereum account for user account
    caller_ether = eth_keys.PrivateKey(operator_keypair.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)
    caller_token = get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID)

    if get_solana_balance(caller) == 0:
        print("Create.caller1 account...")
        evm_loader.create_ether_account(caller_ether)
        print("Done\n")

    print('Account1:', operator_keypair.public_key, bytes(operator_keypair.public_key).hex())
    print("Caller1:", caller_ether.hex(), caller_nonce, "->", caller,
          "({})".format(bytes(PublicKey(caller)).hex()))

    return Caller(operator_keypair, caller_ether, caller, caller_nonce, caller_token)


@pytest.fixture(scope="module")
def caller2(evm_loader) -> Caller:
    wallet = RandomAccount()
    acc2 = Keypair.from_secret_key(wallet.get_acc().secret_key()[:32])

    if get_solana_balance(acc2.public_key) == 0:
        tx = solana_client.request_airdrop(acc2.public_key, 1000000 * 10 ** 9, commitment=Confirmed)
        wait_confirm_transaction(solana_client, tx["result"])

    caller_ether = eth_keys.PrivateKey(acc2.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)
    caller_token = get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID)

    if get_solana_balance(caller) == 0:
        print("Create caller2 account...")
        evm_loader.create_ether_account(caller_ether)
        print("Done\n")

    print('Account2:', acc2.public_key, bytes(acc2.public_key).hex())
    print("Caller2:", caller_ether.hex(), caller_nonce, "->", caller,
          "({})".format(bytes(PublicKey(caller)).hex()))
    yield Caller(acc2, caller_ether, caller, caller_nonce, caller_token)
    os.remove(wallet.path)


@dataclass
class DeployedAccount:
    id: str
    eth: bytes
    code: str


@pytest.fixture(scope="module")
def deployed_acc(evm_loader, caller1) -> DeployedAccount:
    re_id, re_id_eth, re_code = evm_loader.deploy_checked(
        f"{CONTRACTS_DIR}/rw_lock.binary", caller1.address, caller1.ether)
    print('contract', re_id)
    print('contract_eth', re_id_eth.hex())
    print('contract_code', re_code)
    return DeployedAccount(re_id, re_id_eth, re_code)


@dataclass
class CollateralPool:
    index: int
    address: str
    buffer: bytes


@pytest.fixture(scope="module")
def collateral_pool(evm_loader) -> CollateralPool:
    index = 2
    address = create_treasury_pool_address(index)
    index_buf = index.to_bytes(4, 'little')
    return CollateralPool(index, address, index_buf)


class TestRWBlock:
    @pytest.fixture(autouse=True)
    def prepare(self, operator_keypair, evm_loader, caller1, caller2, collateral_pool, deployed_acc):
        self.evm_loader = evm_loader
        self.caller1 = caller1
        self.caller2 = caller2
        self.collateral_pool = collateral_pool
        self.deployed_acc = deployed_acc

    def sol_instr_19_partial_call(self, storage_account, step_count, evm_instruction, writable_code, acc, caller,
                                  add_meta=None):
        add_meta = add_meta or []
        neon_evm_instr_19_partial_call = create_neon_evm_instr_19_partial_call(
            self.evm_loader.loader_id,
            caller,
            acc.public_key,
            storage_account,
            self.deployed_acc.id,
            self.deployed_acc.code,
            self.collateral_pool.buffer,
            self.collateral_pool.address,
            step_count,
            evm_instruction,
            writable_code,
            add_meta,
        )
        print('neon_evm_instr_19_partial_call:', neon_evm_instr_19_partial_call)
        return neon_evm_instr_19_partial_call

    def sol_instr_20_continue(self, storage_account, step_count, writable_code, acc, caller, add_meta=None):
        add_meta = add_meta or []
        neon_evm_instr_20_continue = create_neon_evm_instr_20_continue(
            self.evm_loader.loader_id,
            caller,
            acc.public_key,
            storage_account,
            self.deployed_acc.id,
            self.deployed_acc.code,
            self.collateral_pool.buffer,
            self.collateral_pool.address,
            step_count,
            writable_code,
            add_meta,
        )
        print('neon_evm_instr_20_continue:', neon_evm_instr_20_continue)
        return neon_evm_instr_20_continue

    def neon_evm_instr_cancel_21(self, acc, caller, storage, nonce):
        neon_evm_instr_21_cancel = create_neon_evm_instr_21_cancel(
            self.evm_loader.loader_id,
            caller,
            acc.public_key,
            storage,
            self.deployed_acc.id,
            self.deployed_acc.code,
            nonce
        )
        print('neon_evm_instr_21_cancel:', neon_evm_instr_21_cancel)
        return neon_evm_instr_21_cancel

    def call_begin(self, storage, steps, msg, instruction, writable_code, acc, caller, add_meta=None):
        print("Begin")
        add_meta = add_meta or []
        trx = TransactionWithComputeBudget()
        self.first_instruction_index = len(trx.instructions)
        trx.add(self.sol_instr_keccak(make_keccak_instruction_data(self.first_instruction_index + 1, len(msg), 13)))
        trx.add(self.sol_instr_19_partial_call(storage, steps, instruction, writable_code, acc, caller, add_meta))
        return send_transaction(solana_client, trx, acc)

    def call_continue(self, storage, steps, writable_code, acc, caller, add_meta=None):
        print("Continue")
        add_meta = add_meta or []
        trx = TransactionWithComputeBudget()
        self.first_instruction_index = len(trx.instructions)
        trx.add(self.sol_instr_20_continue(storage, steps, writable_code, acc, caller, add_meta))
        return send_transaction(solana_client, trx, acc)

    def get_call_parameters(self, data, acc: Keypair, caller, caller_ether):
        nonce = get_transaction_count(solana_client, caller)
        tx = {'to': self.deployed_acc.eth, 'value': 0, 'gas': 9999999999, 'gasPrice': 0,
              'nonce': nonce, 'data': data, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, acc.secret_key[:32])
        assert from_addr == caller_ether
        return from_addr, sign, msg, nonce

    def sol_instr_keccak(self, keccak_instruction):
        return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])

    def create_storage_account(self, seed, acc: Keypair):
        storage = PublicKey(
            sha256(bytes(acc.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())
        print("Storage", storage)

        if get_solana_balance(storage) == 0:
            trx = TransactionWithComputeBudget()
            trx.add(create_account_with_seed(acc.public_key, acc.public_key, seed, 10 ** 9, 128 * 1024,
                                             PublicKey(EVM_LOADER)))
            send_transaction(solana_client, trx, acc)

        return storage

    def check_continue_result(self, result):
        if result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']:
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            assert data[0] == 6

    def check_writable(self, res, contract, writable_expected):
        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == contract:
                assert info["writable"] == writable_expected
                return
        raise ("contract_eth not found in  the emulator output, ", self.deployed_acc.eth)

    # the contract account is locked by the read-only lock
    # two transactions of the one contract are executed by two callers
    def test_read_only_blocking(self):
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))

        from_addr1, sign1, msg1, nonce1 = self.get_call_parameters(data, self.caller1.account, self.caller1.address,
                                                                   self.caller1.ether)
        from_addr2, sign2, msg2, nonce2 = self.get_call_parameters(data, self.caller2.account, self.caller2.address,
                                                                   self.caller2.ether)
        print("FIRST PARAMS")
        print(from_addr1, sign1, msg1, nonce1)
        print("SECOND PARAMS")
        print(from_addr2, sign2, msg2, nonce2)

        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        print("INSTRUCTION1 ", instruction1)
        print("INSTRUCTION2 ", instruction2)

        storage1 = self.create_storage_account(sign1[:8].hex(), self.caller1.account)
        storage2 = self.create_storage_account(sign2[1:9].hex(), self.caller1.account)

        print("STORAGE1 ", storage1)
        print("STORAGE2 ", storage2)

        trx = self.call_begin(storage1, 1, msg1, instruction1, False, self.caller1.account, self.caller1.address)
        assert trx["result"]["meta"]["err"] is None
        trx = self.call_begin(storage2, 1, msg2, instruction2, False, self.caller1.account, self.caller2.address)
        assert trx["result"]["meta"]["err"] is None
        result = self.call_continue(storage1, 10, False, self.caller1.account, self.caller1.address)
        result = self.call_continue(storage2, 10, False, self.caller1.account, self.caller2.address)
        result1 = self.call_continue(storage1, 1000, False, self.caller1.account, self.caller1.address)
        result2 = self.call_continue(storage2, 1000, False, self.caller1.account, self.caller2.address)

        self.check_continue_result(result1["result"])
        self.check_continue_result(result2["result"])

        evm_step_executed = 99
        trx_size_cost = 5000
        iterative_overhead = 10_000
        gas = iterative_overhead + trx_size_cost + (evm_step_executed * evm_step_cost())

        for result in [result1["result"], result2["result"]]:
            print('result:', result)
            assert result['meta']['err'] is None
            assert len(result['meta']['innerInstructions']) == 1
            # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
            assert result['meta']['innerInstructions'][0]['index'] == self.first_instruction_index  # second instruction
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            assert data[:1] == b'\x06'  # 6 means OnReturn
            assert data[1] < 0xd0  # less 0xd0 - success
            actual_gas = int().from_bytes(data[2:10], 'little')
            print("actual_gas", actual_gas)
            assert actual_gas == gas  # used_gas
            assert data[10:] == bytes().fromhex("%064x" % 0x2)

    # The first transaction set lock on write to  contract account
    # The second transaction try to set lock on write and  => the error occurs.
    # Then lock removed by Cancel operation
    def test_write_blocking(self):
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1))

        (from_addr1, sign1, msg1, nonce1) = self.get_call_parameters(data, self.caller1.account, self.caller1.address,
                                                                     self.caller1.ether)
        (from_addr2, sign2, msg2, nonce2) = self.get_call_parameters(data, self.caller2.account, self.caller2.address,
                                                                     self.caller2.ether)
        instruction1 = from_addr1 + sign1 + msg1
        instruction2 = from_addr2 + sign2 + msg2

        storage1 = self.create_storage_account(sign1[:8].hex(), self.caller1.account)
        storage2 = self.create_storage_account(sign2[1:9].hex(), self.caller1.account)

        result = self.call_begin(storage1, 10, msg1, instruction1, True, self.caller1.account, self.caller1.address)

        with pytest.raises(RPCException):
            self.call_begin(storage2, 10, msg2, instruction2, True, self.caller1.account, self.caller2.address)

        # removing the rw-lock
        trx = TransactionWithComputeBudget().add(
            self.neon_evm_instr_cancel_21(self.caller1.account, self.caller1.address, storage1, nonce1))
        send_transaction(solana_client, trx, self.caller1.account)

    def test_writable_flag_from_emulator(self):
        print("\ntest_03_writable_flag_from_emulator")

        # 1. "writable" must be False. Storage is not changed
        print("reId_code", self.deployed_acc.code)

        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        res = emulate(self.caller1.ether.hex(), self.deployed_acc.eth.hex(), data.hex(), "")
        self.check_writable(res, self.deployed_acc.eth, False)
        print(res)

        # 2. "writable" must be True. Storage is changed
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        data = (func_name + bytes.fromhex("%064x" % 0x1))
        res = emulate(self.caller1.ether.hex(), self.deployed_acc.eth.hex(), data.hex(), "")
        self.check_writable(res, self.deployed_acc.eth, True)
        print(res)

        # 3. "writable" must be True. Contract nonce is changed
        func_name = abi.function_signature_to_4byte_selector('deploy_contract()')
        res = emulate(self.caller1.ether.hex(), self.deployed_acc.eth.hex(), func_name.hex(), "")
        new_contract_eth = bytes.fromhex(res["result"][-40:])
        self.check_writable(res, self.deployed_acc.eth, True)
        print(res)

        # apply last transaction (the method deploys the contract)
        meta = None
        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == new_contract_eth:
                seed = b58encode(ACCOUNT_SEED_VERSION + new_contract_eth).decode('utf8')
                new_contract_code = account_with_seed(self.caller1.account.public_key, seed, PublicKey(EVM_LOADER))
                create_account_with_seed_in_solana(solana_client, self.caller1.account, self.caller1.account, seed,
                                                   info["code_size"])

                (trx, _) = self.evm_loader.create_ether_account_trx(new_contract_eth, new_contract_code)
                send_transaction(solana_client, trx, self.caller1.account)

                meta = [
                    AccountMeta(pubkey=PublicKey(info["account"]), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=new_contract_code, is_signer=False, is_writable=True),
                ]
                print("new_contract_code", new_contract_code)

        assert meta is not None

        (from_addr, sign, msg, _) = self.get_call_parameters(func_name, self.caller1.account, self.caller1.address,
                                                             self.caller1.ether)
        instruction = from_addr + sign + msg
        storage = self.create_storage_account(sign[:8].hex(), self.caller1.account)

        self.call_begin(storage, 10, msg, instruction, True, self.caller1.account, self.caller1.address, meta)
        self.call_continue(storage, 450, True, self.caller1.account, self.caller1.address, meta)
        result = self.call_continue(storage, 550, True, self.caller1.account, self.caller1.address, meta)
        self.check_continue_result(result["result"])

        # 4. "writable" must be False. Contract calls the method of other contract. Contract nonce is not changed
        func_name = abi.function_signature_to_4byte_selector('call_hello_world()')
        res = emulate(self.caller1.ether.hex(), new_contract_eth.hex(), func_name.hex(), "")
        print(res)
        self.check_writable(res, new_contract_eth, False)

    #  the test must be run last, because it changes contract code account
    #  resizing is blocked  by locking of the account in other transaction.
    def test_resizing_with_account_lock(self):
        print("\ntest_04_resizing_with_account_lock")

        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        input1 = (func_name + bytes.fromhex("%064x" % 0x1))  # update storage without account resizing
        input2 = (func_name + bytes.fromhex("%064x" % 0x20))  # update storage with account resizing

        (from_addr1, sign1, msg1, _) = self.get_call_parameters(input1, self.caller1.account, self.caller1.address,
                                                                self.caller1.ether)
        instruction1 = from_addr1 + sign1 + msg1
        storage1 = self.create_storage_account(sign1[:8].hex(), self.caller1.account)

        # start first transaction
        self.call_begin(storage1, 10, msg1, instruction1, True, self.caller1.account, self.caller1.address)

        # emulate second transaction
        res = emulate(self.caller2.ether.hex(), self.deployed_acc.eth.hex(), input2.hex(), "")
        print(res)
        resize_instr = None

        for info in res["accounts"]:
            address = bytes.fromhex(info["address"][2:])
            if address == self.deployed_acc.eth:
                assert info["writable"] is True
                assert info["code_size"] > info["code_size_current"]

                code_size = info["code_size"] + 2048
                seed_bin = b58encode(ACCOUNT_SEED_VERSION + os.urandom(20))
                seed = seed_bin.decode('utf8')
                code_account_new = account_with_seed(self.caller1.account.public_key, seed, PublicKey(EVM_LOADER))

                print("creating new code_account with increased size %s", code_account_new)
                create_account_with_seed_in_solana(solana_client, self.caller1.account, self.caller1.account, seed,
                                                   code_size)

                resize_instr = TransactionInstruction(
                    keys=[
                        AccountMeta(pubkey=PublicKey(info["account"]), is_signer=False, is_writable=True),
                        AccountMeta(pubkey=info["contract"], is_signer=False, is_writable=True),
                        AccountMeta(pubkey=code_account_new, is_signer=False, is_writable=True),
                        AccountMeta(pubkey=self.caller1.account.public_key, is_signer=True, is_writable=False)
                    ],
                    program_id=EVM_LOADER,
                    data=bytearray.fromhex("11") + bytes(seed_bin)  # 17- ResizeStorageAccount
                )
                break

        assert resize_instr is not None
        # send resizing transaction
        with pytest.raises(Exception, match="invalid instruction data"):
            send_transaction(solana_client, TransactionWithComputeBudget().add(resize_instr), self.caller1.account)

        # get info about resizing account
        info = get_account_data(solana_client, self.deployed_acc.id, ACCOUNT_INFO_LAYOUT.sizeof())
        info_data = AccountInfo.from_bytes(info)

        # resizing must not be completed due to locking contract account.
        assert info_data.code_account == PublicKey(self.deployed_acc.code)

        # finish first transaction for unlocking accounts
        self.call_continue(storage1, 1000, True, self.caller1.account, self.caller1.address)

        # before resizing the old code account must have some balance
        assert get_solana_balance(self.deployed_acc.code) != 0

        # try next attempt to resize storage account and check it
        send_transaction(solana_client, TransactionWithComputeBudget().add(resize_instr), self.caller1.account)
        info = get_account_data(solana_client, self.deployed_acc.id, ACCOUNT_INFO_LAYOUT.sizeof())
        info_data = AccountInfo.from_bytes(info)

        # resizing must be completed => code_account must be updated
        assert info_data.code_account != self.deployed_acc.code

        # after resizing the old code account must have 0 SOL
        assert get_solana_balance(self.deployed_acc.code) == 0
