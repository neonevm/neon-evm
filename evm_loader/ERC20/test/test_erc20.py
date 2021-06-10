from enum import Enum
import base58
import unittest
from solana.rpc import types
from eth_tx_utils import make_keccak_instruction_data, Trx, make_instruction_data_from_tx
from eth_utils import abi
from web3.auto import w3
from solana_utils import *

tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/ERC20/src")


class Mode(Enum):
    NORMAL = 1
    BEGIN = 2
    CONTINUE = 3


class SplToken:
    def __init__(self, url):
        self.url = url

    def call(self, arguments):
        cmd = 'spl-token --url {} {}'.format(self.url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: spl-token error {}".format(err))
            raise


def deploy_erc20(loader, location_hex, location_bin, mintId, balance_erc20, caller):
    ctor_init = str("%064x" % 0xa0) + \
                str("%064x" % 0xe0) + \
                str("%064x" % 0x9) + \
                base58.b58decode(balance_erc20).hex() + \
                base58.b58decode(mintId).hex() + \
                str("%064x" % 0x1) + \
                str("77%062x" % 0x00) + \
                str("%064x" % 0x1) + \
                str("77%062x" % 0x00)

    with open(location_hex, mode='r') as hex:
        binary = bytearray.fromhex(hex.read() + ctor_init)
        with open(location_bin, mode='wb') as bin:
            bin.write(binary)
            res = loader.deploy(location_bin, caller)
            return res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId']


def get_keccak_input(msg, mode=Mode.NORMAL):
    keccak_input = {
        Mode.NORMAL: make_keccak_instruction_data(1, len(msg)),
        Mode.BEGIN: make_keccak_instruction_data(1, len(msg), 9),
        Mode.CONTINUE: make_keccak_instruction_data(1, len(msg), 9),
    }[mode]
    print('keccak_input:', keccak_input.hex(), 'mode:', mode)
    return keccak_input


def get_trx_input(from_addr, sign, msg, step_count, mode=Mode.NORMAL):
    trx_input = {
        Mode.NORMAL: bytes.fromhex("05") + from_addr + sign + msg,
        Mode.BEGIN: bytes.fromhex("09") + step_count.to_bytes(8, byteorder='little') + from_addr + sign + msg,
        Mode.CONTINUE: bytes.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
    }[mode]
    print('trx_input:', trx_input.hex())
    return trx_input


class ExecuteMode(Enum):
    SINGLE = 0
    ITERATIVE = 1


class EtherContract:
    """Encapsulate all data to execute Ether transaction on Solana."""

    def __init__(self, solana_wallet, evm_loader, ether_caller, contract_account, contract_code_account):
        self.mode = ExecuteMode.SINGLE
        self.solana_wallet = solana_wallet
        self.evm_loader = evm_loader
        self.ether_caller = ether_caller
        (self.solana_ether_caller, self.solana_caller_nonce) = self.evm_loader.ether2program(self.ether_caller)
        if getBalance(self.solana_ether_caller) == 0:
            print("Create caller account...")
            self.solana_ether_caller = self.evm_loader.createEtherAccount(self.ether_caller)
            print("Caller account:", self.solana_ether_caller)

        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        self.storage = None

    def set_execute_mode(self, new_mode):
        self.mode = ExecuteMode(new_mode)

    def create_storage_account(self, seed):
        if self.storage is not None:
            return
        self.storage = PublicKey(
            sha256(bytes(self.solana_wallet.public_key())
                   + bytes(seed, 'utf8')
                   + bytes(PublicKey(self.evm_loader.loader_id))).digest())
        print("Storage", self.storage)

        if getBalance(self.storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.solana_wallet.public_key(),
                                          self.solana_wallet.public_key(),
                                          seed, 10 ** 9, 128 * 1024,
                                          PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.solana_wallet)

    def create_instruction_data_from_tx(self, input_data):
        caller_trx_cnt = getTransactionCount(client, self.solana_ether_caller)
        trx_raw = {'to': solana2ether(self.contract_account),
                   'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input_data, 'chainId': 111}
        return make_instruction_data_from_tx(trx_raw, self.solana_wallet.secret_key())

    def call3_single(self, input_data, account_metas=None) -> types.RPCResponse:
        data = bytearray.fromhex("03") + input_data
        print('call_single with data:', data.hex())
        trx = Transaction()
        trx.add(
            TransactionInstruction(program_id=self.evm_loader.loader_id, data=data, keys=
            [
                AccountMeta(pubkey=self.contract_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.contract_code_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.solana_ether_caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.solana_wallet.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.evm_loader.loader_id, is_signer=False, is_writable=False),
            ]))
        if account_metas is not None:
            trx.instructions[-1].keys.extend(account_metas)
        trx.instructions[-1].keys \
            .append(AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False))
        return send_transaction(client, trx, self.solana_wallet)

    def create_trx(self, keccak_data, data):
        print('call_5_create_trx with keccak:', keccak_data.hex(), 'and data:', data.hex())
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=keccakprog, data=keccak_data, keys=
        [
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False),
        ]))
        trx.add(TransactionInstruction(program_id=self.evm_loader.loader_id, data=data, keys=
        [
            AccountMeta(pubkey=self.contract_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.contract_code_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.solana_ether_caller, is_signer=False, is_writable=True),
            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.evm_loader.loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.solana_wallet.public_key(), is_signer=False, is_writable=False),
        ]))
        return trx

    def send_x_transaction(self, x, input_data, account_metas=None, need_storage=False) -> types.RPCResponse:
        (from_address, sign, msg) = self.create_instruction_data_from_tx(input_data)
        keccak_data = make_keccak_instruction_data(1, len(msg), 9 if need_storage else 1)
        data = x + from_address + sign + msg
        trx = self.create_trx(keccak_data, data)
        if need_storage:
            self.create_storage_account(sign[:8].hex())
            trx.instructions[-1].keys.insert(0, AccountMeta(pubkey=self.storage, is_signer=False, is_writable=True))
        if account_metas is not None:
            trx.instructions[-1].keys.extend(account_metas)
        trx.instructions[-1].keys \
            .append(AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False))
        return send_transaction(client, trx, self.solana_wallet)

    def call_5_single(self, input_data, account_metas=None) -> types.RPCResponse:
        return self.send_x_transaction(bytes.fromhex("05")
                                       , input_data, account_metas)

    def call_5_begin(self, input_data, account_metas=None, step_count=100) -> types.RPCResponse:
        return self.send_x_transaction(bytes.fromhex("09") + step_count.to_bytes(8, byteorder='little')
                                       , input_data, account_metas, need_storage=True)

    def call_5_continue(self, input_data, account_metas=None, step_count=100) -> types.RPCResponse:
        return self.send_x_transaction(bytes.fromhex("0A") + step_count.to_bytes(8, byteorder='little')
                                       , input_data, account_metas, need_storage=True)

    def call_5_iterative(self, input_data, account_metas=None, steps=100) -> types.RPCResponse:
        self.call_5_begin(input_data, account_metas, steps)
        while True:
            result = self.call_5_continue(input_data, account_metas, steps)
            if result['result']['meta']['innerInstructions'] \
                    and result['result']['meta']['innerInstructions'][0]['instructions']:
                data = base58.b58decode(result['result']['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if data[0] == 6:
                    self.storage = None
                    return result

    def call_5(self, input_data, account_metas=None) -> types.RPCResponse:
        if self.mode is ExecuteMode.SINGLE:
            return self.call_5_single(input_data, account_metas)
        if self.mode is ExecuteMode.ITERATIVE:
            return self.call_5_iterative(input_data, account_metas)


class ERC20test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = WalletAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoader(cls.wallet, evm_loader_id)

        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()

        print('cls.caller_ether:', cls.caller_ether.hex())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        print('cls.caller:', cls.caller)

        balance = getBalance(cls.acc.public_key())
        print('balance:', balance)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done")
            print("solana caller:", caller, 'cls.caller:', cls.caller)

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        cls.trx_count = getTransactionCount(client, cls.caller)

        erc20_id_ether = keccak_256(rlp.encode((cls.caller_ether, cls.trx_count))).digest()[-20:]
        (cls.erc20Id_precalculated, _) = cls.loader.ether2program(erc20_id_ether)
        print("cls.erc20Id_precalculated:", cls.erc20Id_precalculated)

    @staticmethod
    def createToken(owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("create-token")
        else:
            res = spl.call("create-token --owner {}".format(owner))
        if not res.startswith("Creating token "):
            raise Exception("create token error")
        else:
            return res[15:59]

    @staticmethod
    def createTokenAccount(token, owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("create-account {}".format(token))
        else:
            res = spl.call("create-account {} --owner {}".format(token, owner))
        if not res.startswith("Creating account "):
            raise Exception("create account error %s" % res)
        else:
            return res[17:61]

    @staticmethod
    def tokenMint(mint_id, recipient, amount, owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("mint {} {} {}".format(mint_id, amount, recipient))
        else:
            res = spl.call("mint {} {} {} --owner {}".format(mint_id, amount, recipient, owner))
        print("minting {} tokens for {}".format(amount, recipient))

    @staticmethod
    def tokenBalance(acc):
        spl = SplToken(solana_url)
        res = spl.call("balance --address {}".format(acc))
        return int(res.rstrip())

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

    def erc20_deposit_iterative(self, payer, amount, erc20, erc20_code, balance_erc20, mint_id, receiver_erc20):
        storage = self.erc20_deposit(payer, amount, erc20, erc20_code, balance_erc20, mint_id, receiver_erc20
                                     , Mode.BEGIN, 600)

        while True:
            result = self.erc20_deposit(payer, amount, erc20, erc20_code, balance_erc20, mint_id, receiver_erc20
                                        , Mode.CONTINUE, 600, storage)["result"]
            if result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']:
                data = base58.b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if data[0] == 6:
                    return result

    def erc20_deposit(self, payer, amount, erc20, erc20_code, balance_erc20, mint_id, receiver_erc20,
                      mode=Mode.NORMAL, step_count=100, storage=None):
        func_name = abi.function_signature_to_4byte_selector('deposit(uint256,address,uint256,uint256)')
        input = func_name + bytes.fromhex(
            base58.b58decode(payer).hex() +
            str("%024x" % 0) + receiver_erc20.hex() +
            self.acc.public_key()._key.hex() +
            "%064x" % amount
        )
        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = get_keccak_input(msg, mode)
        trx_input = get_trx_input(from_addr, sign, msg, step_count, mode)

        if mode is Mode.BEGIN:
            storage = self.create_storage_account(sign[:8].hex())

        trx = Transaction();
        if mode is Mode.BEGIN or mode is Mode.NORMAL:
            trx.add(
            TransactionInstruction(program_id=keccakprog,
                                   data=keccak_input,
                                   keys=[
                                       AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False,
                                                   is_writable=False),
                                   ]))

        trx.add(
        TransactionInstruction(program_id=self.loader.loader_id,
                               data=trx_input,
                               keys=[
                                   AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=payer, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                               ]))

        if mode is Mode.BEGIN:
            trx.instructions[-1].keys.insert(0, AccountMeta(pubkey=storage, is_signer=False, is_writable=True))
        if mode is Mode.CONTINUE:
            trx.instructions[0].keys.insert(0, AccountMeta(pubkey=storage, is_signer=False, is_writable=True))

        result = send_transaction(client, trx, self.acc)
        print(result)

        if mode is Mode.BEGIN:
            return storage
        if mode is Mode.CONTINUE:
            return result

        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][2]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert 0 != ret, 'erc20_deposit: FAIL'
        return ret

    def erc20_deposit2(self, ether_contract, payer, amount, balance_erc20, mint_id, receiver_erc20):
        func_name = abi.function_signature_to_4byte_selector('deposit(uint256,address,uint256,uint256)')
        data = func_name + bytes.fromhex(
            base58.b58decode(payer).hex() +
            str("%024x" % 0) + receiver_erc20.hex() +
            self.acc.public_key()._key.hex() +
            "%064x" % amount
        )
        result = ether_contract.call_5_iterative(data, [
            AccountMeta(pubkey=payer, is_signer=False, is_writable=True),
            AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
            AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
        ])
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert 0 != ret, 'erc20_deposit2: FAIL'
        return ret

    def erc20_withdraw_iterative(self, receiver, amount, erc20, erc20_code, balance_erc20, mint_id):
        storage = self.erc20_withdraw(receiver, amount, erc20, erc20_code, balance_erc20, mint_id
                                      , Mode.BEGIN, 600)

        while True:
            result = self.erc20_withdraw(receiver, amount, erc20, erc20_code, balance_erc20, mint_id
                                         , Mode.CONTINUE, 600, storage)["result"]
            if result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']:
                data = base58.b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if data[0] == 6:
                    return result

    def erc20_withdraw(self, receiver, amount, erc20, erc20_code, balance_erc20, mint_id
                       , mode=Mode.NORMAL, step_count=100, storage=None):
        func_name = abi.function_signature_to_4byte_selector('withdraw(uint256,uint256)')
        input = func_name + bytes.fromhex(
            base58.b58decode(receiver).hex() +
            "%064x" % amount
        )
        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = get_keccak_input(msg, mode)
        trx_input = get_trx_input(from_addr, sign, msg, step_count, mode)

        if mode is Mode.BEGIN:
            storage = self.create_storage_account(sign[:8].hex())

        trx = Transaction();
        if mode is Mode.BEGIN or mode is Mode.NORMAL:
            trx.add(
                TransactionInstruction(program_id=keccakprog,
                                       data=keccak_input,
                                       keys=[
                                           AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False,
                                                       is_writable=False),
                                       ]))

        trx.add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=trx_input,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        if mode is Mode.BEGIN:
            trx.instructions[1].keys.insert(0, AccountMeta(pubkey=storage, is_signer=False, is_writable=True))
        if mode is Mode.CONTINUE:
            trx.instructions[0].keys.insert(0, AccountMeta(pubkey=storage, is_signer=False, is_writable=True))

        result = send_transaction(client, trx, self.acc)
        print(result)

        if mode is Mode.BEGIN:
            return storage
        if mode is Mode.CONTINUE:
            return result

        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][2]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert ret != 0, 'erc20_withdraw: FAIL'
        return ret

    def erc20_balance(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('balanceOf(address)')
        input = bytes.fromhex("03") + func_name + bytes.fromhex(
            str("%024x" % 0) +
            self.caller_ether.hex()
        )
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=input,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance = int.from_bytes(value, "big")
        return balance

    def erc20_balance2(self, ether_contract):
        func_name = abi.function_signature_to_4byte_selector('balanceOf(address)')
        data = func_name + bytes.fromhex(
            str("%024x" % 0) +
            ether_contract.ether_caller.hex()
        )
        result = ether_contract.call_5(data)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance = int.from_bytes(value, "big")
        return balance

    def erc20_transfer(self, erc20, erc20_code, eth_to, amount):
        func_name = abi.function_signature_to_4byte_selector('transfer(address,uint256)')
        input = func_name + bytearray.fromhex(
            str("%024x" % 0) + eth_to +
            "%064x" % amount
        )

        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = make_keccak_instruction_data(1, len(msg))
        evm_instruction = from_addr + sign + msg

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_input, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))
        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        print('erc20_transfer:', 'OK' if ret != 0 else 'FAIL')
        return ret

    def erc20_balance_ext(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('balance_ext()')
        input = bytearray.fromhex("03") + func_name
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=input, keys=
            [
                AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance_address = base58.b58encode(value)
        return balance_address

    def erc20_balance_ext2(self, ether_contract):
        func_name = abi.function_signature_to_4byte_selector('balance_ext()')
        result = ether_contract.call3_single(func_name)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance_address = base58.b58encode(value)
        return balance_address

    def erc20_mint_id(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('mint_id()')
        input = bytearray.fromhex("03") + func_name
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=input, keys=
            [
                AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        mint_id = base58.b58encode(value)
        return mint_id

    def erc20_mint_id2(self, ether_contract):
        func_name = abi.function_signature_to_4byte_selector('mint_id()')
        result = ether_contract.call_5(func_name)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        mint_id = base58.b58encode(value)
        return mint_id

    # @unittest.skip("not for CI")
    def test_erc20(self):
        token = self.createToken()
        print("token:", token)

        balance_erc20 = self.createTokenAccount(token, self.erc20Id_precalculated)
        print("balance_erc20:", balance_erc20)

        (erc20Id, erc20Id_ether, erc20_code) = deploy_erc20(self.loader
                                                            , CONTRACTS_DIR + "ERC20.bin"
                                                            , "erc20.binary"
                                                            , token
                                                            , balance_erc20
                                                            , self.caller)
        print("erc20_id:", erc20Id)
        print("erc20_id_ethereum:", erc20Id_ether.hex())
        print("erc20_code:", erc20_code)

        ether_contract = EtherContract(self.acc, self.loader, self.caller_ether, erc20Id, erc20_code)

        assert (self.erc20Id_precalculated == erc20Id)
        assert (balance_erc20 == self.erc20_balance_ext(erc20Id, erc20_code).decode("utf-8"))
        assert (balance_erc20 == self.erc20_balance_ext2(ether_contract).decode("utf-8"))
        assert (token == self.erc20_mint_id(erc20Id, erc20_code).decode("utf-8"))
        assert (token == self.erc20_mint_id2(ether_contract).decode("utf-8"))

        client_acc = self.createTokenAccount(token)
        print("client_acc:", client_acc)

        mint_amount = 100
        self.tokenMint(token, client_acc, mint_amount)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (self.erc20_balance(erc20Id, erc20_code) == 0)
        assert (self.erc20_balance2(ether_contract) == 0)

        deposit_amount = 1
        self.erc20_deposit_iterative(client_acc, deposit_amount * (10 ** 9) >> 1, erc20Id
                                     , erc20_code, balance_erc20, token, self.caller_ether)
        receiver_erc20 = self.caller_ether
        self.erc20_deposit2(ether_contract, client_acc, deposit_amount * (10 ** 9) >> 1
                            , balance_erc20, token, receiver_erc20)
        assert (self.tokenBalance(client_acc) == mint_amount - deposit_amount)
        assert (self.tokenBalance(balance_erc20) == deposit_amount)
        assert (self.erc20_balance(erc20Id, erc20_code) == deposit_amount * (10 ** 9))
        self.erc20_withdraw_iterative(client_acc, deposit_amount * (10 ** 9), erc20Id, erc20_code, balance_erc20, token)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (self.erc20_balance(erc20Id, erc20_code) == 0)

    @unittest.skip("not for CI")
    def test_deposit_iterative(self):
        print("test_deposit")
        client_acc = "BPn4j4UzG4XFFgazYDhVt8wntoyaqRGJbfQPzFBwrBRj"
        erc20_id = "3sKq5KwYzvwB1HAqwrKaohnXTthf5x2qdKhZkc1vGRWf"
        erc20_code = '57JHWkEr7xEAcpgJd1F6x1X1mPF6kTffe9AAaamDvuE2'
        balance_erc20 = "7KdAhUjt9nKgTsw4XjQja9FJ5wQao2JAgBjNfKmtFCJo"
        token = "25AeYuTg2Uey4bYD6D5xjgEmoUXvbjQxZgEKF81p3NUN"
        receiver_erc20 = bytes.fromhex("82374b4598cc62013cba24cf67f8fd38098aa011")
        self.erc20_deposit_iterative(client_acc, 900, erc20_id, erc20_code, balance_erc20, token, receiver_erc20)
        # storage = "6FSYWYck7YYcVS54L7r3uV2GNzKp91UFbN27Z5z95idJ"
        # result = self.erc20_deposit(client_acc, 900, erc20_id, erc20_code, balance_erc20, token, receiver_erc20
        #                             , Mode.CONTINUE, 500, storage)["result"]
        # print('result:', result)

    @unittest.skip("not for CI")
    def test_deposit(self):
        print("test_deposit")
        client_acc = "FzFxJHDaNG2tUUgmgTBjSuZEAA8JpCjmmPmuYN6xRfS2"
        erc20_id = "7sYjkVQhod8Gbbedq2ozZTWCZiRt6cC7K3aKRqpzkLcU"
        erc20_code = 'BhYqykTUWuUCGFKDoVV9aUVEih5Y1LMgRFqQ86YRx35u'
        balance_erc20 = "FwNEpebVFsQ1j54zbrFhWgWVXVa9Xdf5bV94PJ7Du5pN"
        token = "25AeYuTg2Uey4bYD6D5xjgEmoUXvbjQxZgEKF81p3NUN"
        receiver_erc20 = bytes.fromhex("0000000000000000000000000000000000000011")
        self.erc20_deposit(client_acc, 900, erc20_id, erc20_code, balance_erc20, token, receiver_erc20)

    @unittest.skip("not for CI")
    def test_with_draw(self):
        print("test_withdraw")
        client_acc = "297MLscTY5SC4pwpPzTaFQBY4ndHdY1h5jC5FG18RMg2"
        erc20Id = "2a5PhGUpnTsCgVL8TjZ5S3LU76pmUfVC5UBHre4yqs5a"
        balance_erc20 = "8VAcZVoXCQoXb74DGMftRpraMYqHK86qKZALmBopo36i"
        token = "8y9XyppKvAWyu2Ud4HEAH6jaEAcCCvE53wcmr92t9RJJ"
        self.erc20_withdraw(client_acc, 10, erc20Id, balance_erc20, token)

    @unittest.skip("not for CI")
    def test_balance_ext(self):
        print("test_balance_ext")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_balance_ext(erc20Id))

    @unittest.skip("not for CI")
    def test_mint_id(self):
        print("test_mint_id")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_mint_id(erc20Id))

    @unittest.skip("not for CI")
    def test_balance(self):
        print("test_balance")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_balance(erc20Id))

    @unittest.skip("not for CI")
    def test_tranfer(self):
        print("test_transfer")
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        self.erc20_transfer(erc20_id, erc20_code, "0000000000000000000000000000000000000011", 0)


if __name__ == '__main__':
    unittest.main()
