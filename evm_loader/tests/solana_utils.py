import json
import math
import os
import subprocess
import time
import typing
from hashlib import sha256
from typing import NamedTuple, Tuple, Union

import rlp
import spl.token.instructions
from base58 import b58encode
from eth_account.datastructures import SignedTransaction
from eth_keys import keys as eth_keys
from eth_utils import abi
from hexbytes import HexBytes
from sha3 import keccak_256
import solana.system_program as sp

from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.api import Client
from solana.rpc.commitment import Confirmed
from solana.rpc.types import TxOpts
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solders.rpc.responses import SendTransactionResp, GetTransactionResp
from solders.transaction_status import TransactionConfirmationStatus
from spl.token.constants import TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address, ApproveParams

from .utils.constants import EVM_LOADER, SOLANA_URL, SYSTEM_ADDRESS, NEON_TOKEN_MINT_ID, \
    ACCOUNT_SEED_VERSION, TREASURY_POOL_SEED
from .utils.instructions import make_DepositV03, make_Cancel, make_WriteHolder, make_ExecuteTrxFromInstruction, \
    TransactionWithComputeBudget, make_PartialCallOrContinueFromRawEthereumTX, \
    make_ExecuteTrxFromAccountDataIterativeOrContinue
from .utils.layouts import ACCOUNT_INFO_LAYOUT, CREATE_ACCOUNT_LAYOUT
from .utils.types import Caller, Contract

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

    def mint(self, mint_id, recipient, amount, fee_payer=None):
        self.call(
            "mint {} {} --owner evm_loader-keypair.json --fee-payer {} -- {}".format(mint_id, amount, fee_payer,
                                                                                     recipient))
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

    def create_token_account(self, token, owner=None, fee_payer=None):
        if owner is None:
            res = self.call("create-account {}".format(token))
        else:
            res = self.call("create-account {} --owner {} --fee-payer {}".format(token, owner, fee_payer))
        if not res.startswith("Creating account "):
            raise Exception("create account error %s" % res)
        else:
            return res.split()[2]


spl_cli = SplToken(SOLANA_URL)


def create_treasury_pool_address(pool_index, evm_loader=EVM_LOADER):
    return PublicKey.find_program_address(
        [bytes(TREASURY_POOL_SEED, 'utf8'), pool_index.to_bytes(4, 'little')],
        PublicKey(evm_loader)
    )[0]


def wait_confirm_transaction(http_client, tx_sig, confirmations=0):
    """Confirm a transaction."""
    timeout = 30
    elapsed_time = 0
    while elapsed_time < timeout:
        print(f'Get transaction signature for {tx_sig}')
        resp = http_client.get_signature_statuses([tx_sig])
        print(f'Response: {resp}')
        if resp.value[0] is not None:
            status = resp.value[0]
            if status.confirmation_status in [TransactionConfirmationStatus.Finalized,
                                              TransactionConfirmationStatus.Confirmed] and \
                    status.confirmations >= confirmations:
                return
        sleep_time = 1
        time.sleep(sleep_time)
        elapsed_time += sleep_time
    raise RuntimeError("could not confirm transaction: ", tx_sig)


def account_with_seed(base, seed, program) -> PublicKey:
    return PublicKey(sha256(bytes(base) + bytes(seed, 'utf8') + bytes(program)).digest())


def create_account_with_seed(funding, base, seed, lamports, space, program=PublicKey(EVM_LOADER)):
    created = account_with_seed(base, seed, program)
    print(f"Created: {created}")
    return sp.create_account_with_seed(sp.CreateAccountWithSeedParams(
        from_pubkey=funding,
        new_account_pubkey=created,
        base_pubkey=base,
        seed=seed,
        lamports=lamports,
        space=space,
        program_id=program
    ))


def create_holder_account(account, operator):
    return TransactionInstruction(
        keys=[
            AccountMeta(pubkey=account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=operator, is_signer=True, is_writable=False),
        ],
        program_id=PublicKey(EVM_LOADER),
        data=bytes.fromhex("24")
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
        proc_result = subprocess.run(cmd, shell=True, text=True, stdout=subprocess.PIPE, universal_newlines=True)
        result = json.loads(proc_result.stdout)
        if result["result"] == "error":
            error = result["error"]
            raise Exception(f"ERR: neon-cli error {error}")

        proc_result.check_returncode()
        return result["value"]

    def emulate(self, loader_id, sender, contract, data):
        cmd = ["neon-cli",
               "--commitment=recent",
               "--url", SOLANA_URL,
               f"--evm_loader={loader_id}",
               "emulate",
               sender,
               contract
               ]
        print('cmd:', cmd)
        print("data:", data)

        data_json = ""
        if data:
            data_json = f'{{"data": "0x{data}"}}'

        proc_result = subprocess.run(cmd, input=data_json, text=True, stdout=subprocess.PIPE, universal_newlines=True)

        result = json.loads(proc_result.stdout)
        if result["result"] == "error":
            error = result["error"]
            raise Exception(f"ERR: neon-cli error {error}")

        proc_result.check_returncode()
        return result["value"]

    def call_contract_get_function(self, evm_loader, sender, contract, function_signature: str, constructor_args=None):
        data = abi.function_signature_to_4byte_selector(function_signature)
        if constructor_args is not None:
            data += constructor_args
        result = self.emulate(evm_loader.loader_id, sender.eth_address.hex(), contract.eth_address.hex(), data.hex())
        return result["result"]

    def get_steps_count(self, evm_loader, from_acc, to, data):
        if isinstance(to, (Caller, Contract)):
            to = to.eth_address.hex()

        result = neon_cli().emulate(
            evm_loader.loader_id,
            from_acc.eth_address.hex(),
            to,
            data
        )

        return result["steps_executed"]


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
            self.acc = Keypair(d[0:32])

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
    def __init__(self, acc, program_id=EVM_LOADER):
        if program_id is None:
            print(f"EVM Loader program address is empty, deploy it")
            result = json.loads(solana_cli(acc).call('deploy {}'.format(EVM_LOADER_SO)))
            program_id = result['programId']
        EvmLoader.loader_id = PublicKey(program_id)
        print("Done\n")

        self.loader_id = EvmLoader.loader_id
        self.acc = acc
        print("Evm loader program: {}".format(self.loader_id))

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

    def create_ether_account(self, ether):
        (trx, sol) = self.create_ether_account_trx(ether)
        signer = self.acc
        self.check_account(signer.public_key)
        send_transaction(solana_client, trx, signer)
        return sol

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
        acc = account_with_seed(self.acc.public_key, seed, self.loader_id)
        print('ether2program: {} {} => {}'.format(self.ether2hex(ether), 255, acc))
        return acc, 255

    def ether2program(self, ether: Union[str, bytes]) -> Tuple[str, int]:
        items = PublicKey.find_program_address([ACCOUNT_SEED_VERSION, self.ether2bytes(ether)], PublicKey(EVM_LOADER))
        return str(items[0]), items[1]

    def check_account(self, solana):
        info = solana_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))

    def deploy_checked(self, location, caller, caller_ether):
        trx_count = get_transaction_count(solana_client, caller)
        ether = keccak_256(rlp.encode((caller_ether, trx_count))).digest()[-20:]

        program = self.ether2program(ether)
        info = solana_client.get_account_info(PublicKey(program[0]))
        if info.value is None:
            res = self.deploy(location)
            return res['programId'], bytes.fromhex(res['ethereum'][2:])
        elif info.value.owner != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return program[0], ether

    def create_ether_account_trx(self, ether: Union[str, bytes]) -> Tuple[Transaction, str]:
        (sol, nonce) = self.ether2program(ether)
        print('createEtherAccount: {} {} => {}'.format(ether, nonce, sol))
        base = self.acc.public_key
        data = bytes.fromhex('28') + CREATE_ACCOUNT_LAYOUT.build(dict(ether=self.ether2bytes(ether)))
        trx = Transaction()
        trx.add(TransactionInstruction(
            program_id=self.loader_id,
            data=data,
            keys=[
                AccountMeta(pubkey=base, is_signer=True, is_writable=True),
                AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sol), is_signer=False, is_writable=True),
            ]))
        return trx, sol


def get_solana_balance(account):
    return solana_client.get_balance(account, commitment=Confirmed).value


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
    info = info.value
    if info is None:
        raise Exception("Can't get information about {}".format(account))
    if len(info.data) < expected_length:
        print("len(data)({}) < expected_length({})".format(len(info.data), expected_length))
        raise Exception("Wrong data length for account data {}".format(account))
    return info.data


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


def send_transaction(client: Client, trx, acc, wait_status=Confirmed):
    print("Send trx")
    result = client.send_transaction(trx, acc, opts=TxOpts(skip_confirmation=True, preflight_commitment=wait_status))
    tx = result.value
    print("Result: {}".format(result))
    wait_confirm_transaction(client, tx)
    for _ in range(60):
        receipt = client.confirm_transaction(tx)
        if receipt.value is not None:
            break
        time.sleep(1)
    else:
        raise AssertionError(f"Can't get confirmed transaction ")
    return solana_client.get_transaction(tx)


def evm_step_cost():
    operator_expences = PAYMENT_TO_TREASURE + LAMPORTS_PER_SIGNATURE
    return math.floor(operator_expences / EVM_STEPS)


def make_new_user(evm_loader: EvmLoader):
    key = Keypair.generate()
    if get_solana_balance(key.public_key) == 0:
        tx = solana_client.request_airdrop(key.public_key, 1000000 * 10 ** 9, commitment=Confirmed)
        wait_confirm_transaction(solana_client, tx.value)
    caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)
    caller_token = get_associated_token_address(PublicKey(caller), NEON_TOKEN_MINT_ID)

    if get_solana_balance(PublicKey(caller)) == 0:
        print(f"Create Neon account {caller_ether} for user {caller}")
        evm_loader.create_ether_account(caller_ether)

    print('Account solana address:', key.public_key)
    print(f'Account ether address: {caller_ether.hex()} {caller_nonce}', )
    print(f'Account solana address: {caller}')
    return Caller(key, PublicKey(caller), caller_ether, caller_nonce, caller_token)


def deposit_neon(evm_loader: EvmLoader, operator_keypair: Keypair, ether_address: Union[str, bytes], amount: int):
    ether_pubkey, _ether_bump_seed = evm_loader.ether2program(ether_address)

    evm_token_authority, _auth_bump_seed = \
        PublicKey.find_program_address([bytes("Deposit", encoding='utf-8')], evm_loader.loader_id)
    evm_pool_key = get_associated_token_address(evm_token_authority, NEON_TOKEN_MINT_ID)

    signer_token_pubkey = get_associated_token_address(operator_keypair.public_key, NEON_TOKEN_MINT_ID)
    trx = Transaction()
    trx.add(
        spl.token.instructions.approve(
            ApproveParams(
                spl.token.constants.TOKEN_PROGRAM_ID,
                signer_token_pubkey,
                PublicKey(ether_pubkey),
                operator_keypair.public_key,
                amount,
            )
        )
    )
    trx.add(
        make_DepositV03(
            evm_loader.ether2bytes(ether_address),
            PublicKey(ether_pubkey),
            signer_token_pubkey,
            evm_pool_key,
            spl.token.constants.TOKEN_PROGRAM_ID,
            operator_keypair.public_key,
        )
    )

    receipt = send_transaction(solana_client, trx, operator_keypair)

    return receipt


def cancel_transaction(
        tx_hash: HexBytes,
        holder_acc: PublicKey,
        operator_keypair: Keypair,
        additional_accounts: typing.List[PublicKey],
):
    # Cancel deployment transaction:
    trx = Transaction()
    trx.add(
        make_Cancel(
            holder_acc,
            operator_keypair,
            tx_hash,
            additional_accounts,
        )
    )

    cancel_receipt = send_transaction(solana_client, trx, operator_keypair)

    print("Cancel receipt:", cancel_receipt)
    assert cancel_receipt.value.transaction.meta.err is None
    return cancel_receipt


def write_transaction_to_holder_account(
        signed_tx: SignedTransaction,
        holder_account: PublicKey,
        operator: Keypair,
):
    # Write transaction to transaction holder account
    offset = 0
    receipts = []
    rest = signed_tx.rawTransaction
    while len(rest):
        (part, rest) = (rest[:920], rest[920:])
        trx = Transaction()
        trx.add(make_WriteHolder(operator.public_key, holder_account, signed_tx.hash, offset, part))
        receipts.append(
            solana_client.send_transaction(
                trx,
                operator,
                opts=TxOpts(skip_confirmation=True, preflight_commitment=Confirmed),
            )
        )
        offset += len(part)
    for rcpt in receipts:
        wait_confirm_transaction(solana_client, rcpt.value)


def execute_trx_from_instruction(operator: Keypair, evm_loader, treasury_address: PublicKey, treasury_buffer: bytes,
                                 instruction: SignedTransaction,
                                 additional_accounts, signer: Keypair, system_program=sp.SYS_PROGRAM_ID,
                                 evm_loader_public_key=PublicKey(EVM_LOADER)) -> SendTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(make_ExecuteTrxFromInstruction(operator, evm_loader, treasury_address,
                                           treasury_buffer, instruction.rawTransaction, additional_accounts,
                                           system_program, evm_loader_public_key))

    return solana_client.send_transaction(trx, signer, opts=TxOpts(skip_preflight=False, skip_confirmation=False))


def send_transaction_step_from_instruction(operator: Keypair, evm_loader, treasury, storage_account,
                                           instruction: SignedTransaction,
                                           additional_accounts, steps_count, signer: Keypair,
                                           system_program=sp.SYS_PROGRAM_ID,
                                           evm_loader_public_key=PublicKey(EVM_LOADER)) -> SendTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(
        make_PartialCallOrContinueFromRawEthereumTX(
            instruction.rawTransaction,
            operator, evm_loader, storage_account, treasury.account, treasury.buffer, steps_count,
            additional_accounts, system_program, evm_loader_public_key
        )
    )

    return solana_client.send_transaction(trx, signer, opts=TxOpts(skip_preflight=False, skip_confirmation=False))


def execute_transaction_steps_from_instruction(operator: Keypair, evm_loader, treasury, storage_account,
                                               instruction: SignedTransaction,
                                               additional_accounts, steps_count=EVM_STEPS,
                                               signer: Keypair = None) -> SendTransactionResp:
    signer = operator if signer is None else signer

    send_transaction_step_from_instruction(operator, evm_loader, treasury, storage_account, instruction,
                                           additional_accounts, 1, signer)
    if steps_count > 0:
        steps_left = steps_count
        while steps_left > 0:
            send_transaction_step_from_instruction(operator, evm_loader, treasury, storage_account, instruction,
                                                   additional_accounts, EVM_STEPS, signer)
            steps_left = steps_left - EVM_STEPS
    return send_transaction_step_from_instruction(operator, evm_loader, treasury, storage_account, instruction,
                                                  additional_accounts, 1, signer)


def send_transaction_step_from_account(operator: Keypair, evm_loader, treasury, storage_account,
                                       additional_accounts, steps_count, signer: Keypair,
                                       system_program=sp.SYS_PROGRAM_ID,
                                       evm_loader_public_key=PublicKey(EVM_LOADER), tag=33) -> GetTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(
        make_ExecuteTrxFromAccountDataIterativeOrContinue(
            operator, evm_loader, storage_account, treasury.account, treasury.buffer, steps_count,
            additional_accounts, system_program, evm_loader_public_key, tag
        )
    )
    return send_transaction(solana_client, trx, signer)


def execute_transaction_steps_from_account(operator: Keypair, evm_loader, treasury, storage_account,
                                           additional_accounts, steps_count=EVM_STEPS,
                                           signer: Keypair = None) -> GetTransactionResp:
    signer = operator if signer is None else signer

    send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, 1, signer)
    if steps_count > 0:
        steps_left = steps_count
        while steps_left > 0:
            send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts,
                                               EVM_STEPS, signer)
            steps_left = steps_left - EVM_STEPS
    return send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, 1,
                                              signer)


def execute_transaction_steps_from_account_no_chain_id(operator: Keypair, evm_loader, treasury, storage_account,
                                                       additional_accounts, steps_count=EVM_STEPS,
                                                       signer: Keypair = None) -> GetTransactionResp:
    signer = operator if signer is None else signer

    send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, 1, signer,
                                       tag=34)
    if steps_count > 0:
        steps_left = steps_count
        while steps_left > 0:
            send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts,
                                               EVM_STEPS, signer, tag=34)
            steps_left = steps_left - EVM_STEPS
    return send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, 1,
                                              signer, tag=34)
