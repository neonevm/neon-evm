from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.api import Client
from solana.rpc.types import TxOpts
from solana.rpc import types
from solana.account import Account
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
import time
import os
import subprocess
from typing import NamedTuple
import json
from eth_keys import keys as eth_keys
import base64
from base58 import b58encode
from solana._layouts.system_instructions import SYSTEM_INSTRUCTIONS_LAYOUT, InstructionType as SystemInstructionType
from construct import Bytes, Int8ul, Int64ul, Struct as cStruct
from hashlib import sha256
from sha3 import keccak_256
import rlp
from enum import Enum
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
import base58

CREATE_ACCOUNT_LAYOUT = cStruct(
    "lamports" / Int64ul,
    "space" / Int64ul,
    "ether" / Bytes(20),
    "nonce" / Int8ul
)

system = "11111111111111111111111111111111"
tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
EVM_LOADER = os.environ.get("EVM_LOADER")

EVM_LOADER_SO = os.environ.get("EVM_LOADER_SO", 'target/bpfel-unknown-unknown/release/evm_loader.so')
client = Client(solana_url)
path_to_solana = 'solana'


class SplToken:
    def __init__(self, url):
        self.url = url

    def call(self, arguments):
        cmd = 'spl-token --url {} {}'.format(self.url, arguments)
        print('cmd:', cmd)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: spl-token error {}".format(err))
            raise

    def balance(self, acc):
        res = self.call("balance --address {}".format(acc))
        return int(res.rstrip())

    def mint(self, mint_id, recipient, amount, owner=None):
        if owner is None:
            self.call("mint {} {} {}".format(mint_id, amount, recipient))
        else:
            self.call("mint {} {} {} --owner {}".format(mint_id, amount, recipient, owner))
        print("minting {} tokens for {}".format(amount, recipient))

    def create_token(self, owner=None):
        if owner is None:
            res = self.call("create-token")
        else:
            res = self.call("create-token --owner {}".format(owner))
        if not res.startswith("Creating token "):
            raise Exception("create token error")
        else:
            return res.split()[2]

    def create_token_account(self, token, owner=None):
        if owner is None:
            res = self.call("create-account {}".format(token))
        else:
            res = self.call("create-account {} --owner {}".format(token, owner))
        if not res.startswith("Creating account "):
            raise Exception("create account error %s" % res)
        else:
            return res.split()[2]


class EthereumTransaction:
    """Encapsulate the all data of an ethereum transaction that should be executed."""

    def __init__(self, ether_caller, contract_account, contract_code_account, trx_data, account_metas=None, steps=500):
        self.ether_caller = ether_caller
        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        self.trx_data = trx_data
        self.trx_account_metas = account_metas
        self.iterative_steps = steps
        self._solana_ether_caller = None  # is created in NeonEvmClient.__create_instruction_data_from_tx
        self._storage = None  # is created in NeonEvmClient.__send_neon_transaction
        print('trx_data:', self.trx_data.hex())
        if self.trx_account_metas is not None:
            print('trx_account_metas:', *self.trx_account_metas, sep='\n')


class ExecuteMode(Enum):
    SINGLE = 0
    ITERATIVE = 1


class NeonEvmClient:
    """Encapsulate the interaction logic with evm_loader to execute an ethereum transaction."""

    def __init__(self, solana_wallet, evm_loader):
        self.mode = ExecuteMode.SINGLE
        self.solana_wallet = solana_wallet
        self.evm_loader = evm_loader

    def set_execute_mode(self, new_mode):
        self.mode = ExecuteMode(new_mode)

    def send_ethereum_trx(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        if self.mode is ExecuteMode.SINGLE:
            return self.send_ethereum_trx_single(ethereum_transaction)
        if self.mode is ExecuteMode.ITERATIVE:
            return self.send_ethereum_trx_iterative(ethereum_transaction)

    def send_ethereum_trx_iterative(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        self.__send_neon_transaction(bytes.fromhex("09") +
                                     ethereum_transaction.iterative_steps.to_bytes(8, byteorder='little'),
                                     ethereum_transaction, need_storage=True)
        while True:
            result = self.__send_neon_transaction(bytes.fromhex("0A") +
                                                  ethereum_transaction.iterative_steps.to_bytes(8, byteorder='little'),
                                                  ethereum_transaction, need_storage=True)
            if result['result']['meta']['innerInstructions'] \
                    and result['result']['meta']['innerInstructions'][0]['instructions']:
                data = base58.b58decode(result['result']['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if data[0] == 6:
                    ethereum_transaction.__storage = None
                    return result

    def send_ethereum_trx_single(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        return self.__send_neon_transaction(bytes.fromhex("05"), ethereum_transaction)

    def __create_solana_ether_caller(self, ethereum_transaction):
        caller = self.evm_loader.ether2program(ethereum_transaction.ether_caller)[0]
        if ethereum_transaction._solana_ether_caller is None \
                or ethereum_transaction._solana_ether_caller != caller:
            ethereum_transaction._solana_ether_caller = caller
        if getBalance(ethereum_transaction._solana_ether_caller) == 0:
            print("Create solana ether caller account...")
            ethereum_transaction._solana_ether_caller = \
                self.evm_loader.createEtherAccount(ethereum_transaction.ether_caller)
        print("Solana ether caller account:", ethereum_transaction._solana_ether_caller)

    def __create_storage_account(self, seed):
        storage = PublicKey(
            sha256(bytes(self.solana_wallet.public_key())
                   + bytes(seed, 'utf8')
                   + bytes(PublicKey(self.evm_loader.loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.solana_wallet.public_key(),
                                          self.solana_wallet.public_key(),
                                          seed, 10 ** 9, 128 * 1024,
                                          PublicKey(EVM_LOADER)))
            send_transaction(client, trx, self.solana_wallet)
        return storage

    def __create_instruction_data_from_tx(self, ethereum_transaction):
        self.__create_solana_ether_caller(ethereum_transaction)
        caller_trx_cnt = getTransactionCount(client, ethereum_transaction._solana_ether_caller)
        trx_raw = {'to': solana2ether(ethereum_transaction.contract_account),
                   'value': 1, 'gas': 9999999, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': ethereum_transaction.trx_data, 'chainId': 111}
        return make_instruction_data_from_tx(trx_raw, self.solana_wallet.secret_key())

    def __create_trx(self, ethereum_transaction, keccak_data, data):
        print('create_trx with keccak:', keccak_data.hex(), 'and data:', data.hex())
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=PublicKey(keccakprog), data=keccak_data, keys=
        [
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False),
        ]))
        trx.add(TransactionInstruction(program_id=self.evm_loader.loader_id, data=data, keys=
        [
            AccountMeta(pubkey=ethereum_transaction.contract_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=ethereum_transaction.contract_code_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=ethereum_transaction._solana_ether_caller, is_signer=False, is_writable=True),
            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.evm_loader.loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.solana_wallet.public_key(), is_signer=False, is_writable=False),
        ]))
        return trx

    def __send_neon_transaction(self, evm_trx_data, ethereum_transaction, need_storage=False) -> types.RPCResponse:
        (from_address, sign, msg) = self.__create_instruction_data_from_tx(ethereum_transaction)
        keccak_data = make_keccak_instruction_data(1, len(msg), 9 if need_storage else 1)
        data = evm_trx_data + from_address + sign + msg
        trx = self.__create_trx(ethereum_transaction, keccak_data, data)
        if need_storage:
            if ethereum_transaction._storage is None:
                ethereum_transaction._storage = self.__create_storage_account(sign[:8].hex())
            trx.instructions[-1].keys \
                .insert(0, AccountMeta(pubkey=ethereum_transaction._storage, is_signer=False, is_writable=True))
        if ethereum_transaction.trx_account_metas is not None:
            trx.instructions[-1].keys.extend(ethereum_transaction.trx_account_metas)
        trx.instructions[-1].keys \
            .append(AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False))
        return send_transaction(client, trx, self.solana_wallet)


def confirm_transaction(http_client, tx_sig, confirmations=1):
    """Confirm a transaction."""
    TIMEOUT = 30  # 30 seconds  pylint: disable=invalid-name
    elapsed_time = 0
    while elapsed_time < TIMEOUT:
        print('confirm_transaction for %s', tx_sig)
        resp = http_client.get_signature_statuses([tx_sig])
        print('confirm_transaction: %s', resp)
        if resp["result"]:
            status = resp['result']['value'][0]
            if status and (status['confirmationStatus'] == 'finalized' or status['confirmationStatus'] == 'confirmed'
                           and status['confirmations'] >= confirmations):
                return
        sleep_time = 1
        time.sleep(sleep_time)
        elapsed_time += sleep_time
    raise RuntimeError("could not confirm transaction: ", tx_sig)


def accountWithSeed(base, seed, program):
    print(type(base), type(seed), type(program))
    return PublicKey(sha256(bytes(base) + bytes(seed, 'utf8') + bytes(program)).digest())

def createAccountWithSeed(funding, base, seed, lamports, space, program):
    data = SYSTEM_INSTRUCTIONS_LAYOUT.build(
        dict(
            instruction_type=SystemInstructionType.CreateAccountWithSeed,
            args=dict(
                base=bytes(base),
                seed=dict(length=len(seed), chars=seed),
                lamports=lamports,
                space=space,
                program_id=bytes(program)
            )
        )
    )
    print("createAccountWithSeed", data.hex())
    created = accountWithSeed(base, seed,
                              program)  # PublicKey(sha256(bytes(base)+bytes(seed, 'utf8')+bytes(program)).digest())
    print("created", created)
    return TransactionInstruction(
        keys=[
            AccountMeta(pubkey=funding, is_signer=True, is_writable=True),
            AccountMeta(pubkey=created, is_signer=False, is_writable=True),
            AccountMeta(pubkey=base, is_signer=True, is_writable=False),
        ],
        program_id=system,
        data=data
    )


class solana_cli:
    def __init__(self, acc=None):
        self.acc = acc

    def call(self, arguments):
        cmd = ""
        if self.acc == None:
            cmd = '{} --url {} {}'.format(path_to_solana, solana_url, arguments)
        else:
            cmd = '{} --keypair {} --url {} {}'.format(path_to_solana, self.acc.get_path(), solana_url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise


class neon_cli:
    def call(self, arguments):
        cmd = 'neon-cli --url {} {}'.format(solana_url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: neon-cli error {}".format(err))
            raise

    def emulate(self, loader_id, arguments):
        cmd = 'neon-cli  --commitment=recent --evm_loader {} --url {} emulate {}'.format(loader_id,
                                                                                         solana_url,
                                                                                         arguments)
        print('cmd:', cmd)
        try:
            output = subprocess.check_output(cmd, shell=True, universal_newlines=True)
            without_empty_lines = os.linesep.join([s for s in output.splitlines() if s])
            last_line = without_empty_lines.splitlines()[-1]
            return last_line
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: neon-cli error {}".format(err))
            raise


class RandomAccount:
    def __init__(self, path=None):
        if path == None:
            self.make_random_path()
            print("New keypair file: {}".format(self.path))
            self.generate_key()
        else:
            self.path = path
        self.retrieve_keys()
        print('New Public key:', self.acc.public_key())
        print('Private:', self.acc.secret_key())

    def make_random_path(self):
        self.path  = os.urandom(5).hex()+ ".json"

    def generate_key(self):
        cmd_generate = 'solana-keygen new --no-passphrase --outfile {}'.format(self.path)
        try:
            return subprocess.check_output(cmd_generate, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise

    def retrieve_keys(self):
        with open(self.path) as f:
            d = json.load(f)
            self.acc = Account(d[0:32])

    def get_path(self):
        return self.path

    def get_acc(self):
        return self.acc


class WalletAccount(RandomAccount):
    def __init__(self, path):
        self.path = path
        self.retrieve_keys()
        print('Wallet public key:', self.acc.public_key())


class EvmLoader:
    def __init__(self, acc, programId=EVM_LOADER):
        if programId == None:
            print("Load EVM loader...")
            result = json.loads(solana_cli(acc).call('deploy {}'.format(EVM_LOADER_SO)))
            programId = result['programId']
        EvmLoader.loader_id = programId
        print("Done\n")

        self.loader_id = EvmLoader.loader_id
        self.acc = acc
        print("Evm loader program: {}".format(self.loader_id))

    def deploy(self, contract_path, caller=None, config=None):
        print('deploy caller:', caller)
        if config == None:
            output = neon_cli().call("deploy --evm_loader {} {} {}".format(self.loader_id, contract_path, caller))
        else:
            output = neon_cli().call("deploy --evm_loader {} --config {} {} {}".format(self.loader_id, config,
                                                                                       contract_path, caller))
        print(type(output), output)
        result = json.loads(output.splitlines()[-1])
        return result

    def createEtherAccount(self, ether):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else:
            ether = ether.hex()
        (sol, nonce) = self.ether2program(ether)
        print('createEtherAccount: {} {} => {}'.format(ether, nonce, sol))
        trx = Transaction()
        base = self.acc.get_acc().public_key()
        trx.add(TransactionInstruction(
            program_id=self.loader_id,
            data=bytes.fromhex('02000000') + CREATE_ACCOUNT_LAYOUT.build(dict(
                lamports=10 ** 9,
                space=0,
                ether=bytes.fromhex(ether),
                nonce=nonce)),
            keys=[
                AccountMeta(pubkey=base, is_signer=True, is_writable=False),
                AccountMeta(pubkey=PublicKey(sol), is_signer=False, is_writable=True),
                AccountMeta(pubkey=system, is_signer=False, is_writable=False),
            ]))
        result = send_transaction(client, trx, self.acc.get_acc())
        print('result:', result)
        return sol

    def ether2seed(self, ether):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else:
            ether = ether.hex()
        seed = b58encode(bytes.fromhex(ether)).decode('utf8')
        acc = accountWithSeed(self.acc.get_acc().public_key(), seed, PublicKey(self.loader_id))
        print('ether2program: {} {} => {}'.format(ether, 255, acc))
        return (acc, 255)

    def ether2program(self, ether):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else:
            ether = ether.hex()
        output = neon_cli().call("create-program-address --evm_loader {} {}".format(self.loader_id, ether))
        items = output.rstrip().split(' ')
        return items[0], int(items[1])

    def checkAccount(self, solana):
        info = client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))

    def deployChecked(self, location, caller, caller_ether):
        trx_count = getTransactionCount(client, caller)
        ether = keccak_256(rlp.encode((caller_ether, trx_count))).digest()[-20:]

        program = self.ether2program(ether)
        code = self.ether2seed(ether)
        info = client.get_account_info(program[0])
        if info['result']['value'] is None:
            res = self.deploy(location, caller)
            return res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId']
        elif info['result']['value']['owner'] != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return program[0], ether, code[0]

    def createEtherAccountTrx(self, ether, code_acc=None):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else:
            ether = ether.hex()
        (sol, nonce) = self.ether2program(ether)
        print('createEtherAccount: {} {} => {}'.format(ether, nonce, sol))
        seed = b58encode(bytes.fromhex(ether))
        base = self.acc.get_acc().public_key()
        data = bytes.fromhex('02000000') + CREATE_ACCOUNT_LAYOUT.build(dict(
            lamports=10 ** 9,
            space=0,
            ether=bytes.fromhex(ether),
            nonce=nonce))
        trx = Transaction()
        if code_acc is None:
            trx.add(TransactionInstruction(
                program_id=self.loader_id,
                data=data,
                keys=[
                    AccountMeta(pubkey=base, is_signer=True, is_writable=True),
                    AccountMeta(pubkey=PublicKey(sol), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=system, is_signer=False, is_writable=False),
                ]))
        else:
            trx.add(TransactionInstruction(
                program_id=self.loader_id,
                data=data,
                keys=[
                    AccountMeta(pubkey=base, is_signer=True, is_writable=True),
                    AccountMeta(pubkey=PublicKey(sol), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=PublicKey(code_acc), is_signer=False, is_writable=True),
                    AccountMeta(pubkey=system, is_signer=False, is_writable=False),
                ]))
        return (trx, sol)


def getBalance(account):
    return client.get_balance(account, commitment=Confirmed)['result']['value']


def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])


ACCOUNT_INFO_LAYOUT = cStruct(
    "type" / Int8ul,
    "eth_acc" / Bytes(20),
    "nonce" / Int8ul,
    "trx_count" / Bytes(8),
    "code_acc" / Bytes(32),
    "is_blocked" / Int8ul,
    "blocked_by" / Bytes(32),
)


class AccountInfo(NamedTuple):
    eth_acc: eth_keys.PublicKey
    trx_count: int

    @staticmethod
    def frombytes(data):
        cont = ACCOUNT_INFO_LAYOUT.parse(data)
        return AccountInfo(cont.eth_acc, cont.trx_count)


def getAccountData(client, account, expected_length):
    info = client.get_account_info(account, commitment=Confirmed)['result']['value']
    if info is None:
        raise Exception("Can't get information about {}".format(account))

    data = base64.b64decode(info['data'][0])
    if len(data) != expected_length:
        print("len(data)({}) != expected_length({})".format(len(data), expected_length))
        raise Exception("Wrong data length for account data {}".format(account))
    return data


def getTransactionCount(client, sol_account):
    info = getAccountData(client, sol_account, ACCOUNT_INFO_LAYOUT.sizeof())
    acc_info = AccountInfo.frombytes(info)
    res = int.from_bytes(acc_info.trx_count, 'little')
    print('getTransactionCount {}: {}'.format(sol_account, res))
    return res


def wallet_path():
    res = solana_cli().call("config get")
    substr = "Keypair Path: "
    for line in res.splitlines():
        if line.startswith(substr):
            return line[len(substr):].strip()
    raise Exception("cannot get keypair path")


def send_transaction(client, trx, acc):
    result = client.send_transaction(trx, acc, opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))
    confirm_transaction(client, result["result"])
    result = client.get_confirmed_transaction(result["result"])
    return result
