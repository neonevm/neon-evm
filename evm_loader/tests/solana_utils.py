import base64
import json
import os
import subprocess
import math
import time
import pathlib
from hashlib import sha256
from typing import NamedTuple, Tuple, Union

import rlp
from base58 import b58encode

from eth_keys import keys as eth_keys
from sha3 import keccak_256
from solana._layouts.system_instructions import SYSTEM_INSTRUCTIONS_LAYOUT, InstructionType as SystemInstructionType
from solana.account import Account
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.api import Client
from solana.rpc.commitment import Confirmed, Processed
from solana.rpc.types import TxOpts
from solana.system_program import SYS_PROGRAM_ID
from solana.transaction import AccountMeta, TransactionInstruction, Transaction

from spl.token.constants import TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address, approve, ApproveParams, create_associated_token_account

from .utils.instructions import TransactionWithComputeBudget
from .utils.constants import EVM_LOADER, SOLANA_URL, TREASURY_POOL_BASE, SYSTEM_ADDRESS, NEON_TOKEN_MINT_ID, \
    SYS_INSTRUCT_ADDRESS, INCINERATOR_ADDRESS, ACCOUNT_SEED_VERSION
from .utils.layouts import ACCOUNT_INFO_LAYOUT
from .utils.types import Caller


EVM_LOADER_SO = os.environ.get("EVM_LOADER_SO", 'target/bpfel-unknown-unknown/release/evm_loader.so')
solana_client = Client(SOLANA_URL)
path_to_solana = 'solana'

# amount of gas per 1 byte evm_storage
EVM_BYTE_COST = 6960  # 1_000_000_000/ 100 * 365 / (1024*1024) * 2
# number of evm steps per transaction
EVM_STEPS = 500
# the message size that is used to holder-account filling
HOLDER_MSG_SIZE = 950
# Ethereum account allocated data size
ACCOUNT_MAX_SIZE = 256
# spl-token account allocated data size
SPL_TOKEN_ACCOUNT_SIZE = 165
# payment to treasure
PAYMENT_TO_TREASURE = 5000
# payment for solana signature verification
LAMPORTS_PER_SIGNATURE = 5000
# account storage overhead for calculation of base rent
ACCOUNT_STORAGE_OVERHEAD = 128


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

    def transfer(self, mint, amount, recipient):
        self.call("transfer {} {} {}".format(mint, amount, recipient))

    def balance(self, acc):
        from decimal import Decimal
        res = self.call("balance --address {}".format(acc))
        return Decimal(res.rstrip())

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


spl_cli = SplToken(SOLANA_URL)


def create_treasury_pool_address(collateral_pool_index):
    collateral_seed_prefix = "collateral_seed_"
    seed = collateral_seed_prefix + str(collateral_pool_index)
    return account_with_seed(PublicKey(TREASURY_POOL_BASE), seed, PublicKey(EVM_LOADER))


def wait_confirm_transaction(http_client, tx_sig, confirmations=0):
    """Confirm a transaction."""
    timeout = 30
    elapsed_time = 0
    while elapsed_time < timeout:
        print(f'Get transaction signature for {tx_sig}')
        resp = http_client.get_signature_statuses([tx_sig])
        print(f'Response: {resp}')
        if resp["result"]:
            status = resp['result']['value'][0]
            if status and (status['confirmationStatus'] == 'finalized' or status['confirmationStatus'] == 'confirmed'
                           and status['confirmations'] >= confirmations):
                return
        sleep_time = 1
        time.sleep(sleep_time)
        elapsed_time += sleep_time
    raise RuntimeError("could not confirm transaction: ", tx_sig)


def account_with_seed(base, seed, program) -> PublicKey:
    return PublicKey(sha256(bytes(base) + bytes(seed, 'utf8') + bytes(program)).digest())


def create_account_with_seed(funding, base, seed, lamports, space, program=PublicKey(EVM_LOADER)):
    data = SYSTEM_INSTRUCTIONS_LAYOUT.build(
        dict(
            instruction_type=SystemInstructionType.CREATE_ACCOUNT_WITH_SEED,
            args=dict(
                base=bytes(base),
                seed=dict(length=len(seed), chars=seed),
                lamports=lamports,
                space=space,
                program_id=bytes(program)
            )
        )
    )
    print(f"Create account with seed, data = {data.hex()}")
    created = account_with_seed(base, seed, program)
    print(f"Created: {created}")
    return TransactionInstruction(
        keys=[
            AccountMeta(pubkey=funding, is_signer=True, is_writable=True),
            AccountMeta(pubkey=created, is_signer=False, is_writable=True),
            AccountMeta(pubkey=base, is_signer=True, is_writable=False),
        ],
        program_id=PublicKey(SYSTEM_ADDRESS),
        data=data
    )


class solana_cli:
    def __init__(self, acc=None):
        self.acc = acc

    def call(self, arguments):
        if self.acc is None:
            cmd = '{} --url {} {}'.format(path_to_solana, SOLANA_URL, arguments)
        else:
            cmd = '{} --keypair {} --url {} {}'.format(path_to_solana, self.acc.get_path(), SOLANA_URL, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            print(f"ERR: solana error {err}")
            raise


class neon_cli:
    def __init__(self, verbose_flags=''):
        self.verbose_flags = verbose_flags

    def call(self, arguments):
        cmd = 'neon-cli {} --commitment=processed --url {} {} -vvv'.format(self.verbose_flags, SOLANA_URL, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            print(f"ERR: neon-cli error {err}")
            raise

    def emulate(self, loader_id, arguments):
        cmd = 'neon-cli {} --commitment=processed --evm_loader {} --url {} emulate {}'.format(self.verbose_flags,
                                                                                              loader_id,
                                                                                              SOLANA_URL,
                                                                                              arguments)
        print('cmd:', cmd)
        try:
            output = subprocess.check_output(cmd, shell=True, universal_newlines=True)
            without_empty_lines = os.linesep.join([s for s in output.splitlines() if s])
            last_line = without_empty_lines.splitlines()[-1]
            return last_line
        except subprocess.CalledProcessError as err:
            print(f"ERR: neon-cli error {err}")
            raise


class RandomAccount:
    def __init__(self, path=None):
        if path is None:
            self.make_random_path()
            print(f"New keypair file: {self.path}")
            self.generate_key()
        else:
            self.path = path
        self.retrieve_keys()
        print('New Public key:', self.acc.public_key())
        print('Private:', self.acc.secret_key())

    def make_random_path(self):
        self.path = os.urandom(5).hex() + ".json"

    def generate_key(self):
        cmd_generate = 'solana-keygen new --no-passphrase --outfile {}'.format(self.path)
        try:
            return subprocess.check_output(cmd_generate, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            print(f"ERR: solana error {err}")
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


class OperatorAccount:
    def __init__(self, path=None):
        if path is None:
            self.path = pathlib.Path.home() / ".config" / "solana" / "id.json"
        else:
            self.path = path
        self.retrieve_keys()

    def retrieve_keys(self):
        with open(self.path) as f:
            d = json.load(f)
            self.acc = Account(d[0:32])

    def get_path(self):
        return self.path

    def get_acc(self):
        return self.acc


class EvmLoader:
    def __init__(self, acc: OperatorAccount, program_id=EVM_LOADER):
        if program_id is None:
            print(f"EVM Loader program address is empty, deploy it")
            result = json.loads(solana_cli(acc).call('deploy {}'.format(EVM_LOADER_SO)))
            program_id = result['programId']
        EvmLoader.loader_id = program_id
        print("Done\n")

        self.loader_id = EvmLoader.loader_id
        self.acc = acc
        print("Evm loader program: {}".format(self.loader_id))

    def airdrop_neon_tokens(self, user_ether_address: Union[str, bytes], amount: int) -> None:
        operator = self.acc.get_acc()

        (neon_evm_authority, _) = PublicKey.find_program_address([b"Deposit"], PublicKey(self.loader_id))
        pool_token_account = get_associated_token_address(neon_evm_authority, NEON_TOKEN_MINT_ID)
        source_token_account = get_associated_token_address(operator.public_key(), NEON_TOKEN_MINT_ID)
        (user_solana_address, _) = self.ether2program(user_ether_address)

        pool_account_exists = solana_client.get_account_info(
            pool_token_account, commitment=Processed
        )["result"]["value"] is not None
        print("Pool Account Exists: ", pool_account_exists)

        trx = TransactionWithComputeBudget()
        if not pool_account_exists:
            trx.add(create_associated_token_account(operator.public_key(), neon_evm_authority, NEON_TOKEN_MINT_ID))

        trx.add(approve(ApproveParams(
            program_id=TOKEN_PROGRAM_ID,
            source=source_token_account,
            delegate=neon_evm_authority,
            owner=operator.public_key(),
            amount=amount * (10 ** 9),
        )))
        trx.add(TransactionInstruction(
            program_id=self.loader_id,
            data=bytes.fromhex("1e") + self.ether2bytes(user_ether_address),
            keys=[
                AccountMeta(pubkey=source_token_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=pool_token_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=PublicKey(user_solana_address), is_signer=False, is_writable=True),
                AccountMeta(pubkey=neon_evm_authority, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=operator.public_key(), is_signer=True, is_writable=True),
                AccountMeta(pubkey=SYS_PROGRAM_ID, is_signer=False, is_writable=True),
            ]
        ))
        result = send_transaction(solana_client, trx, operator)
        print("Airdrop transaction: ", result)

    def deploy(self, contract_path, config=None):
        print(f'Deploy contract from path: {contract_path}')
        if config is None:
            output = neon_cli().call("deploy --evm_loader {} {}".format(self.loader_id, contract_path))
        else:
            output = neon_cli().call("deploy --evm_loader {} --config {} {}".format(self.loader_id, config,
                                                                                    contract_path))
        print(f"Deploy output: {output}")
        result = json.loads(output.splitlines()[-1])
        return result

    @staticmethod
    def ether2hex(ether: Union[str, bytes]):
        if isinstance(ether, str):
            if ether.startswith('0x'):
                return ether[2:]
            return ether
        return ether.hex()

    @staticmethod
    def ether2bytes(ether: Union[str, bytes]):
        if isinstance(ether, str):
            if ether.startswith('0x'):
                return bytes.fromhex(ether[2:])
            return bytes.fromhex(ether)
        return ether

    def ether2seed(self, ether: Union[str, bytes]):
        seed = b58encode(ACCOUNT_SEED_VERSION + self.ether2bytes(ether)).decode('utf8')
        acc = account_with_seed(self.acc.get_acc().public_key(), seed, PublicKey(self.loader_id))
        print('ether2program: {} {} => {}'.format(self.ether2hex(ether), 255, acc))
        return acc, 255

    def ether2program(self, ether: Union[str, bytes]):
        output = neon_cli().call("create-program-address --evm_loader {} {}".format(self.loader_id, self.ether2hex(ether)))
        items = output.rstrip().split(' ')
        return items[0], int(items[1])

    def check_account(self, solana):
        info = solana_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))

    def deploy_checked(self, location, caller, caller_ether):
        trx_count = get_transaction_count(solana_client, caller)
        ether = keccak_256(rlp.encode((caller_ether, trx_count))).digest()[-20:]

        program = self.ether2program(ether)
        code = self.ether2seed(ether)
        info = solana_client.get_account_info(program[0])
        if info['result']['value'] is None:
            res = self.deploy(location)
            return res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId']
        elif info['result']['value']['owner'] != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return program[0], ether, code[0]


def get_solana_balance(account):
    return solana_client.get_balance(account, commitment=Confirmed)['result']['value']


class AccountInfo(NamedTuple):
    ether: eth_keys.PublicKey
    trx_count: int

    @staticmethod
    def from_bytes(data: bytes):
        cont = ACCOUNT_INFO_LAYOUT.parse(data)
        return AccountInfo(cont.ether, cont.trx_count)


def get_account_data(solana_client: Client, account: Union[str, PublicKey, Keypair], expected_length: int) -> bytes:
    if isinstance(account, Keypair):
        account = account.public_key
    print(f"Get account data for {account}")
    info = solana_client.get_account_info(account, commitment=Confirmed)
    print(f"Result: {info}")
    info = info['result']['value']
    if info is None:
        raise Exception("Can't get information about {}".format(account))

    data = base64.b64decode(info['data'][0])
    if len(data) < expected_length:
        print("len(data)({}) < expected_length({})".format(len(data), expected_length))
        raise Exception("Wrong data length for account data {}".format(account))
    return data


def get_transaction_count(solana_client: Client, sol_account: Union[str, PublicKey]) -> int:
    info = get_account_data(solana_client, sol_account, ACCOUNT_INFO_LAYOUT.sizeof())
    acc_info = AccountInfo.from_bytes(info)
    res = int.from_bytes(acc_info.trx_count, 'little')
    print('getTransactionCount {}: {}'.format(sol_account, res))
    return res


def get_neon_balance(solana_client: Client, sol_account: Union[str, PublicKey]) -> int:
    info = get_account_data(solana_client, sol_account, ACCOUNT_INFO_LAYOUT.sizeof())
    account = ACCOUNT_INFO_LAYOUT.parse(info)
    balance = int.from_bytes(account.balance, byteorder="little")
    print('getNeonBalance {}: {}'.format(sol_account, balance))
    return balance


def send_transaction(client, trx, acc, wait_status=Confirmed):
    print("Send trx")
    result = client.send_transaction(trx, acc, opts=TxOpts(skip_confirmation=True, preflight_commitment=wait_status))
    tx = result["result"]
    print("Result: {}".format(result))
    wait_confirm_transaction(client, tx)
    for _ in range(6):
        receipt = client.get_confirmed_transaction(tx)
        if receipt["result"] is not None:
            break
        time.sleep(10)
    else:
        raise AssertionError(f"Can't get confirmed transaction ")
    return receipt


def create_neon_evm_instr_05_single(evm_loader_program_id,
                                    caller_sol_acc,
                                    operator_sol_acc,
                                    contract_sol_acc,
                                    code_sol_acc,
                                    collateral_pool_index_buf,
                                    collateral_pool_address,
                                    evm_instruction,
                                    add_meta=None):
    if add_meta is None:
        add_meta = []
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("05") + collateral_pool_index_buf + evm_instruction,
        keys=[
                 # System instructions account:
                 AccountMeta(pubkey=PublicKey(SYS_INSTRUCT_ADDRESS), is_signer=False, is_writable=False),

                 # Operator's SOL account:
                 AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
                 # Collateral pool address:
                 AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
                 # Operator's NEON account:
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
                 # System program account:
                 AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                 # NeonEVM program account
                 AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

                 AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),

             ] + add_meta + [
                 AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
             ])


def create_neon_evm_instr_13_partial_call_or_continue(evm_loader_program_id,
                                                      caller_sol_acc,
                                                      operator_sol_acc,
                                                      storage_sol_acc,
                                                      contract_sol_acc,
                                                      code_sol_acc,
                                                      collateral_pool_index_buf,
                                                      collateral_pool_address,
                                                      step_count,
                                                      evm_instruction,
                                                      writable_code=True,
                                                      add_meta=None):
    if add_meta is None:
        add_meta = []
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("0D") + collateral_pool_index_buf + step_count.to_bytes(8,
                                                                                       byteorder='little') + evm_instruction,
        keys=[
                 AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),
                 # System instructions account:
                 AccountMeta(pubkey=PublicKey(SYS_INSTRUCT_ADDRESS), is_signer=False, is_writable=False),

                 # Operator's SOL account:
                 AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
                 # Collateral pool address:
                 AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
                 # Operator's NEON account:
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
                 # System program account:
                 AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                 # NeonEVM program account
                 AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

                 AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=writable_code),
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),

                 AccountMeta(pubkey=PublicKey(SYS_INSTRUCT_ADDRESS), is_signer=False, is_writable=False),
             ] + add_meta + [
                 AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
             ])


def create_neon_evm_instr_19_partial_call(evm_loader_program_id,
                                          caller_sol_acc,
                                          operator_sol_acc,
                                          storage_sol_acc,
                                          contract_sol_acc,
                                          code_sol_acc,
                                          collateral_pool_index_buf,
                                          collateral_pool_address,
                                          step_count,
                                          evm_instruction,
                                          writable_code=True,
                                          add_meta=None):
    if add_meta is None:
        add_meta = []
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("13") + collateral_pool_index_buf + step_count.to_bytes(8,
                                                                                       byteorder='little') + evm_instruction,
        keys=[
                 AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),
                 # System instructions account:
                 AccountMeta(pubkey=PublicKey(SYS_INSTRUCT_ADDRESS), is_signer=False, is_writable=False),

                 # Operator's SOL account:
                 AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
                 # Collateral pool address:
                 AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
                 # Operator's NEON account:
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
                 # System program account:
                 AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                 # NeonEVM program account
                 AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

                 AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=writable_code),
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
             ] + add_meta + [
                 AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
             ])


def create_neon_evm_instr_20_continue(evm_loader_program_id,
                                      caller_sol_acc,
                                      operator_sol_acc,
                                      storage_sol_acc,
                                      contract_sol_acc,
                                      code_sol_acc,
                                      collateral_pool_index_buf,
                                      collateral_pool_address,
                                      step_count,
                                      writable_code=True,
                                      add_meta=None):
    if add_meta is None:
        add_meta = []
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("14") + collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little'),
        keys=[
                 # Operator's storage account:
                 AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),
                 # Operator's SOL account:
                 AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
                 # Collateral pool address:
                 AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
                 # Operator's NEON account:
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
                 # System program account:
                 AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                 # NeonEVM program account
                 AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

                 AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
                 AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=writable_code),
                 AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
             ] + add_meta + [
                 AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
             ])


def create_neon_evm_instr_22_begin(evm_loader_program_id,
                                   caller_sol_acc,
                                   operator_sol_acc,
                                   storage_sol_acc,
                                   holder_sol_acc,
                                   contract_sol_acc,
                                   code_sol_acc,
                                   collateral_pool_index_buf,
                                   collateral_pool_address,
                                   step_count):
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("16") + collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little'),
        keys=[
            AccountMeta(pubkey=holder_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),

            # Operator's SOL account:
            AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
            # Collateral pool address:
            AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
            # Operator's NEON token account:
            AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
            # System program account:
            AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
            # NeonEVM program account
            AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

            AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),

            AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
        ])


def create_neon_evm_instr_21_cancel(evm_loader_program_id,
                                    caller_sol_acc,
                                    operator_sol_acc,
                                    storage_sol_acc,
                                    contract_sol_acc,
                                    code_sol_acc,
                                    nonce):
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("15") + nonce.to_bytes(8, 'little'),
        keys=[
            AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),

            # Operator's SOL account:
            AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
            # Incinerator
            AccountMeta(pubkey=PublicKey(INCINERATOR_ADDRESS), is_signer=False, is_writable=True),

            AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),

            AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
        ])


def create_neon_evm_instr_14_combined_continue(evm_loader_program_id,
                                               caller_sol_acc,
                                               operator_sol_acc,
                                               storage_sol_acc,
                                               holder_sol_acc,
                                               contract_sol_acc,
                                               code_sol_acc,
                                               collateral_pool_index_buf,
                                               collateral_pool_address,
                                               step_count):
    return TransactionInstruction(
        program_id=evm_loader_program_id,
        data=bytearray.fromhex("0E") + collateral_pool_index_buf + step_count.to_bytes(8, byteorder='little'),
        keys=[
            AccountMeta(pubkey=holder_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=storage_sol_acc, is_signer=False, is_writable=True),

            # Operator's SOL account:
            AccountMeta(pubkey=operator_sol_acc, is_signer=True, is_writable=True),
            # Collateral pool address:
            AccountMeta(pubkey=collateral_pool_address, is_signer=False, is_writable=True),
            # Operator's NEON account:
            AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),
            # System program account:
            AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
            # NeonEVM program account
            AccountMeta(pubkey=evm_loader_program_id, is_signer=False, is_writable=False),

            AccountMeta(pubkey=contract_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=code_sol_acc, is_signer=False, is_writable=True),
            AccountMeta(pubkey=caller_sol_acc, is_signer=False, is_writable=True),

            AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
        ])


def evm_step_cost():
    operator_expences = PAYMENT_TO_TREASURE + LAMPORTS_PER_SIGNATURE
    return math.floor(operator_expences / EVM_STEPS)


def make_new_user(evm_loader: EvmLoader):
    key = Keypair.generate()
    if get_solana_balance(key.public_key) == 0:
        tx = solana_client.request_airdrop(key.public_key, 1000000 * 10 ** 9, commitment=Confirmed)
        wait_confirm_transaction(solana_client, tx["result"])
    caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)
    caller_token = get_associated_token_address(PublicKey(caller), NEON_TOKEN_MINT_ID)

    if get_solana_balance(caller) == 0:
        print(f"Create account for user {caller}")
        evm_loader.airdrop_neon_tokens(caller_ether, 1)

    print('Account solana address:', key.public_key)
    print(f'Account ether address: {caller_ether.hex()} {caller_nonce}', )
    print(f'Account solana address: {caller}')
    return Caller(key, caller, caller_ether, caller_nonce, caller_token)
