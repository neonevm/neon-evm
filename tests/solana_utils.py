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
from spl.token.instructions import get_associated_token_address, ApproveParams, MintToParams

from .utils.constants import CHAIN_ID

from .utils.constants import EVM_LOADER, SOLANA_URL, SYSTEM_ADDRESS, NEON_TOKEN_MINT_ID, \
    ACCOUNT_SEED_VERSION, TREASURY_POOL_SEED
from .utils.instructions import make_DepositV03, make_Cancel, make_WriteHolder, make_ExecuteTrxFromInstruction, \
    TransactionWithComputeBudget, make_PartialCallOrContinueFromRawEthereumTX, \
    make_ExecuteTrxFromAccountDataIterativeOrContinue, make_CreateAssociatedTokenIdempotent
from .utils.layouts import BALANCE_ACCOUNT_LAYOUT, CONTRACT_ACCOUNT_LAYOUT
from .utils.types import Caller, Contract

EVM_LOADER_SO = os.environ.get("EVM_LOADER_SO", 'target/bpfel-unknown-unknown/release/evm_loader.so')
solana_client = Client(SOLANA_URL, commitment=Confirmed)
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



def create_treasury_pool_address(pool_index, evm_loader=EVM_LOADER):
    return PublicKey.find_program_address(
        [bytes(TREASURY_POOL_SEED, 'utf8'), pool_index.to_bytes(4, 'little')],
        PublicKey(evm_loader)
    )[0]


def wait_for_account_to_exists(http_client: Client, account: PublicKey, timeout = 30, sleep_time = 0.4):
    elapsed_time = 0
    while elapsed_time < timeout:
        resp = http_client.get_account_info(account, commitment=Confirmed)
        if resp.value is not None:
            return

        time.sleep(sleep_time)
        elapsed_time += sleep_time
        
    raise RuntimeError(f"Account {account} not exists after {timeout} seconds")


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


def create_holder_account(account, operator, seed):
    return TransactionInstruction(
        keys=[
            AccountMeta(pubkey=account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=operator, is_signer=True, is_writable=False),
        ],
        program_id=PublicKey(EVM_LOADER),
        data=bytes.fromhex("24") + 
            len(seed).to_bytes(8, 'little') + seed
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
        cmd = 'neon-cli {} --loglevel debug --commitment=processed --url {} {}'.format(self.verbose_flags, SOLANA_URL, arguments)
        proc_result = subprocess.run(cmd, shell=True, text=True, stdout=subprocess.PIPE, universal_newlines=True)
        result = json.loads(proc_result.stdout)
        if result["result"] == "error":
            error = result["error"]
            raise Exception(f"ERR: neon-cli error {error}")

        proc_result.check_returncode()
        return result["value"]

    def emulate(self, loader_id, sender, contract, data):
        cmd = ["neon-cli",
               "--commitment=confirmed",
               "--url", SOLANA_URL,
               f"--evm_loader={loader_id}",
               "emulate"
               ]
        print('cmd:', cmd)
        print("data:", data)

        body = json.dumps({
            "tx": {
                "from": sender,
                "to": contract,
                "data": data
            },
            "accounts": []
        })

        proc_result = subprocess.run(cmd, input=body, text=True, stdout=subprocess.PIPE, universal_newlines=True)

        result = json.loads(proc_result.stdout)
        print("EMULATOR RESULT: ")
        print(json.dumps(result))

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
    def __init__(self, acc: Keypair, program_id=EVM_LOADER):
        if program_id is None:
            print(f"EVM Loader program address is empty, deploy it")
            result = json.loads(solana_cli(acc).call('deploy {}'.format(EVM_LOADER_SO)))
            program_id = result['programId']
        EvmLoader.loader_id = PublicKey(program_id)
        print("Done\n")

        self.loader_id = EvmLoader.loader_id
        self.acc = acc
        print("Evm loader program: {}".format(self.loader_id))

    def create_balance_account(self, ether: Union[str, bytes]) -> PublicKey:
        account_pubkey = self.ether2balance(ether)
        contract_pubkey = PublicKey(self.ether2program(ether)[0])
        print('createBalanceAccount: {} => {}'.format(ether, account_pubkey))

        data = bytes([0x30]) + self.ether2bytes(ether) + CHAIN_ID.to_bytes(8, 'little')
        trx = Transaction()
        trx.add(TransactionInstruction(
            program_id=self.loader_id,
            data=data,
            keys=[
                AccountMeta(pubkey=self.acc.public_key, is_signer=True, is_writable=True),
                AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
                AccountMeta(pubkey=account_pubkey, is_signer=False, is_writable=True),
                AccountMeta(pubkey=contract_pubkey, is_signer=False, is_writable=True),
            ]))

        send_transaction(solana_client, trx, self.acc)
        return account_pubkey

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
    
    def ether2balance(self, address: Union[str, bytes]) -> PublicKey:
        address_bytes = self.ether2bytes(address)
        chain_id_bytes = CHAIN_ID.to_bytes(32, 'big')
        return PublicKey.find_program_address(
            [ACCOUNT_SEED_VERSION, address_bytes, chain_id_bytes],
            PublicKey(EVM_LOADER)
        )[0]

    def check_account(self, solana):
        info = solana_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))
        
    def get_neon_nonce(self, account: Union[str, bytes, Caller]) -> int:
        if isinstance(account, Caller):
            return self.get_neon_nonce(account.eth_address)

        solana_address = self.ether2balance(account)

        info: bytes = get_solana_account_data(solana_client, solana_address, BALANCE_ACCOUNT_LAYOUT.sizeof())
        layout = BALANCE_ACCOUNT_LAYOUT.parse(info)

        return layout.trx_count

    def get_neon_balance(self, account: Union[str, bytes, Caller]) -> int:
        if isinstance(account, Caller):
            return self.get_neon_balance(account.eth_address)
        
        solana_address = self.ether2balance(account)

        info: bytes = get_solana_account_data(solana_client, solana_address, BALANCE_ACCOUNT_LAYOUT.sizeof())
        layout = BALANCE_ACCOUNT_LAYOUT.parse(info)

        return int.from_bytes(layout.balance, byteorder="little")


def get_solana_balance(account):
    return solana_client.get_balance(account, commitment=Confirmed).value


def get_solana_account_data(solana_client: Client, account: Union[str, PublicKey, Keypair], expected_length: int) -> bytes:
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

def send_transaction(client: Client, trx: Transaction, *signers: Keypair, wait_status=Confirmed):
    print("Send trx")
    result = client.send_transaction(trx, *signers, opts=TxOpts(skip_confirmation=True, preflight_commitment=wait_status))
    print("Result: {}".format(result))
    client.confirm_transaction(result.value, commitment=Confirmed)
    return client.get_transaction(result.value, commitment=Confirmed)


def evm_step_cost():
    operator_expences = PAYMENT_TO_TREASURE + LAMPORTS_PER_SIGNATURE
    return math.floor(operator_expences / EVM_STEPS)


def make_new_user(evm_loader: EvmLoader) -> Caller:
    key = Keypair.generate()
    if get_solana_balance(key.public_key) == 0:
        solana_client.request_airdrop(key.public_key, 1000 * 10 ** 9, commitment=Confirmed)
        wait_for_account_to_exists(solana_client, key.public_key)

    caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
    caller_solana = evm_loader.ether2program(caller_ether)[0]
    caller_balance = evm_loader.ether2balance(caller_ether)
    caller_token = get_associated_token_address(caller_balance, NEON_TOKEN_MINT_ID)

    if get_solana_balance(caller_balance) == 0:
        print(f"Create Neon account {caller_ether} for user {caller_balance}")
        evm_loader.create_balance_account(caller_ether)

    print('Account solana address:', key.public_key)
    print(f'Account ether address: {caller_ether.hex()}', )
    print(f'Account solana address: {caller_balance}')
    return Caller(key, PublicKey(caller_solana), caller_balance, caller_ether, caller_token)


def deposit_neon(evm_loader: EvmLoader, operator_keypair: Keypair, ether_address: Union[str, bytes], amount: int):
    balance_pubkey = evm_loader.ether2balance(ether_address)
    contract_pubkey = PublicKey(evm_loader.ether2program(ether_address)[0])

    evm_token_authority = PublicKey.find_program_address([b"Deposit"], evm_loader.loader_id)[0]
    evm_pool_key = get_associated_token_address(evm_token_authority, NEON_TOKEN_MINT_ID)

    token_pubkey = get_associated_token_address(operator_keypair.public_key, NEON_TOKEN_MINT_ID)

    with open("evm_loader-keypair.json", "r") as key:
        secret_key = json.load(key)[:32]
        mint_authority = Keypair.from_secret_key(secret_key)

    trx = Transaction()
    trx.add(
        make_CreateAssociatedTokenIdempotent(
            operator_keypair.public_key,
            operator_keypair.public_key,
            NEON_TOKEN_MINT_ID
        ),
        spl.token.instructions.mint_to(
            MintToParams(
                spl.token.constants.TOKEN_PROGRAM_ID,
                NEON_TOKEN_MINT_ID,
                token_pubkey,
                mint_authority.public_key,
                amount
            )
        ),
        spl.token.instructions.approve(
            ApproveParams(
                spl.token.constants.TOKEN_PROGRAM_ID,
                token_pubkey,
                balance_pubkey,
                operator_keypair.public_key,
                amount,
            )
        ),
        make_DepositV03(
            evm_loader.ether2bytes(ether_address),
            CHAIN_ID,
            balance_pubkey,
            contract_pubkey,
            NEON_TOKEN_MINT_ID,
            token_pubkey,
            evm_pool_key,
            spl.token.constants.TOKEN_PROGRAM_ID,
            operator_keypair.public_key,
        )
    )

    receipt = send_transaction(solana_client, trx, operator_keypair, mint_authority)

    return receipt


def cancel_transaction(
        evm_loader: EvmLoader,
        tx_hash: HexBytes,
        holder_acc: PublicKey,
        operator_keypair: Keypair,
        additional_accounts: typing.List[PublicKey],
):
    # Cancel deployment transaction:
    trx = Transaction()
    trx.add(
        make_Cancel(
            evm_loader,
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
        solana_client.confirm_transaction(rcpt.value, commitment=Confirmed)


def execute_trx_from_instruction(operator: Keypair, evm_loader: EvmLoader, treasury_address: PublicKey, treasury_buffer: bytes,
                                 instruction: SignedTransaction,
                                 additional_accounts, signer: Keypair, 
                                 system_program=sp.SYS_PROGRAM_ID) -> SendTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(make_ExecuteTrxFromInstruction(operator, evm_loader, treasury_address,
                                           treasury_buffer, instruction.rawTransaction, additional_accounts,
                                           system_program))

    return solana_client.send_transaction(trx, signer, opts=TxOpts(skip_preflight=False, skip_confirmation=False, preflight_commitment=Confirmed))


def send_transaction_step_from_instruction(operator: Keypair, evm_loader: EvmLoader, treasury, storage_account,
                                           instruction: SignedTransaction,
                                           additional_accounts, steps_count, signer: Keypair,
                                           system_program=sp.SYS_PROGRAM_ID, index=0) -> SendTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(
        make_PartialCallOrContinueFromRawEthereumTX(
            index, steps_count, instruction.rawTransaction,
            operator, evm_loader, storage_account, treasury,
            additional_accounts, system_program
        )
    )

    return solana_client.send_transaction(trx, signer, opts=TxOpts(skip_preflight=False, skip_confirmation=False, preflight_commitment=Confirmed))


def execute_transaction_steps_from_instruction(operator: Keypair, evm_loader: EvmLoader, treasury, storage_account,
                                               instruction: SignedTransaction,
                                               additional_accounts, steps_count=EVM_STEPS,
                                               signer: Keypair = None) -> SendTransactionResp:
    signer = operator if signer is None else signer

    index = 0
    done = False
    while not done:
        response = send_transaction_step_from_instruction(operator, evm_loader, treasury, storage_account, instruction, additional_accounts, EVM_STEPS, signer, index=index)
        index += 1

        receipt = solana_client.get_transaction(response.value, commitment=Confirmed)
        if receipt.value.transaction.meta.err:
            raise AssertionError(f"Can't deploy contract: {receipt.value.transaction.meta.err}")
        for log in receipt.value.transaction.meta.log_messages:
            if "exit_status" in log:
                done = True
                break
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
            
    return response


def send_transaction_step_from_account(operator: Keypair, evm_loader: EvmLoader, treasury, storage_account,
                                       additional_accounts, steps_count, signer: Keypair,
                                       system_program=sp.SYS_PROGRAM_ID,
                                       tag=0x35, index=0) -> GetTransactionResp:
    trx = TransactionWithComputeBudget(operator)
    trx.add(
        make_ExecuteTrxFromAccountDataIterativeOrContinue(
            index, steps_count,
            operator, evm_loader, storage_account, treasury,
            additional_accounts, system_program, tag
        )
    )
    return send_transaction(solana_client, trx, signer)


def execute_transaction_steps_from_account(operator: Keypair, evm_loader: EvmLoader, treasury, storage_account,
                                           additional_accounts, steps_count=EVM_STEPS, signer: Keypair = None) -> GetTransactionResp:
    signer = operator if signer is None else signer

    index = 0
    done = False
    while not done:
        receipt = send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, EVM_STEPS, signer, index=index)
        index += 1

        if receipt.value.transaction.meta.err:
            raise AssertionError(f"Can't deploy contract: {receipt.value.transaction.meta.err}")
        for log in receipt.value.transaction.meta.log_messages:
            if "exit_status" in log:
                done = True
                break
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
            
    return receipt


def execute_transaction_steps_from_account_no_chain_id(operator: Keypair, evm_loader: EvmLoader, treasury, storage_account,
                                                       additional_accounts, steps_count=EVM_STEPS,
                                                       signer: Keypair = None) -> GetTransactionResp:
    signer = operator if signer is None else signer

    index = 0
    done = False
    while not done:
        receipt = send_transaction_step_from_account(operator, evm_loader, treasury, storage_account, additional_accounts, EVM_STEPS, signer, tag=0x36, index=index)
        index += 1

        if receipt.value.transaction.meta.err:
            raise AssertionError(f"Can't deploy contract: {receipt.value.transaction.meta.err}")
        for log in receipt.value.transaction.meta.log_messages:
            if "exit_status" in log:
                done = True
                break
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
            
    return receipt
