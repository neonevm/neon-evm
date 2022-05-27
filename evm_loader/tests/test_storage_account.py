import os
import pathlib
import typing as tp
from hashlib import sha256
from base58 import b58decode, b58encode
from dataclasses import dataclass

import pytest
from solana.rpc.core import RPCException
from solana.transaction import AccountMeta, TransactionInstruction
from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.system_program import SYS_PROGRAM_ID
from solana.rpc.commitment import Confirmed
from spl.token.constants import TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi
from eth_keys import keys as eth_keys
from solana_utils import account_with_seed, EVM_LOADER, sysinstruct, solana_client, TransactionWithComputeBudget, \
    create_account_with_seed, send_transaction, get_solana_balance, \
    wait_confirm_transaction, ETH_TOKEN_MINT_ID, create_treasury_pool_address, RandomAccount, \
    create_neon_evm_instr_19_partial_call, create_neon_evm_instr_20_continue, create_neon_evm_instr_21_cancel, \
    get_transaction_count, keccakprog, ACCOUNT_SEED_VERSION, get_account_data, AccountInfo, ACCOUNT_INFO_LAYOUT, \
    evm_step_cost


CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR")
if CONTRACTS_DIR is None:
    CONTRACTS_DIR = pathlib.Path(__file__).parent / "contracts"


@dataclass
class Caller:
    solana_account: Keypair
    solana_account_address: PublicKey
    ether_address: bytes
    nonce: int
    token_address: PublicKey


@pytest.fixture(scope="module")
def user_account(evm_loader) -> Caller:
    # Create ethereum account for user account
    key = Keypair.generate()
    if get_solana_balance(key.public_key) == 0:
        tx = solana_client.request_airdrop(key.public_key, 1000000 * 10 ** 9, commitment=Confirmed)
        wait_confirm_transaction(solana_client, tx["result"])
    caller_ether = eth_keys.PrivateKey(key.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)
    caller_token = get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID)

    if get_solana_balance(caller) == 0:
        print(f"Create account for user {caller}")
        evm_loader.create_ether_account(caller_ether)

    print('Account:', key.public_key)
    print("Caller1:", caller_ether.hex(), caller_nonce, "->", caller,
          "({})".format(bytes(PublicKey(caller)).hex()))

    return Caller(key, caller, caller_ether, caller_nonce, caller_token)


@dataclass
class DeployedContract:
    program: PublicKey
    eth_address: bytes
    code: PublicKey


@pytest.fixture(scope="module")
def deploy_contract(evm_loader, user_account) -> DeployedContract:
    program, contract_address, code = evm_loader.deploy_checked(
        f"{CONTRACTS_DIR}/rw_lock.binary", user_account.solana_account_address, user_account.ether_address)
    return DeployedContract(PublicKey(program), contract_address, PublicKey(code))


@dataclass
class TreasuryPool:
    index: int
    account: PublicKey
    buffer: bytes


@pytest.fixture(scope="module")
def treasury_pool(evm_loader) -> TreasuryPool:
    index = 2
    address = create_treasury_pool_address(index)
    index_buf = index.to_bytes(4, 'little')
    return TreasuryPool(index, address, index_buf)


def make_eth_transaction(signer: Keypair, from_solana_account: PublicKey, to_address: tp.Union[str, bytes], from_address: tp.Union[str, bytes], data: bytes):
    nonce = get_transaction_count(solana_client, from_solana_account)
    tx = {'to': to_address, 'value': 0, 'gas': 9999999999, 'gasPrice': 0,
          'nonce': nonce, 'data': data, 'chainId': 111}
    print("ETH transaction body: ", tx)
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, signer.secret_key[:32])
    assert from_addr == from_address
    return from_addr, sign, msg, nonce


def create_storage_account(seed, signer: Keypair) -> PublicKey:
    print(f"Create storage account with seed: {seed}")
    storage = PublicKey(
        sha256(bytes(signer.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())

    if get_solana_balance(storage) == 0:
        trx = TransactionWithComputeBudget()
        trx.add(create_account_with_seed(signer.public_key, signer.public_key, seed, 10 ** 9, 128 * 1024,
                                         PublicKey(EVM_LOADER)))
        send_transaction(solana_client, trx, signer)
    print(f"Storage account: {storage}")
    return storage


def make_PartialCallOrContinueFromRawEthereumTX_transaction(treasury_index: bytes,
                                                            step_count: int,
                                                            msg: bytes,
                                                            operator: Keypair,
                                                            storage_account: PublicKey,
                                                            treasury_account: PublicKey,
                                                            operator_eth_account: PublicKey,
                                                            program_first_account: PublicKey,
                                                            program_second_account: PublicKey,
                                                            user_account: PublicKey,
                                                            ):
    print("Make PartialCallOrContinueFromRawEthereumTX transaction")
    data = bytearray.fromhex("0d") + treasury_index + step_count.to_bytes(8, 'little') + msg
    print(f"Input data: {data}")
    trx = TransactionWithComputeBudget()
    trx.add(
        TransactionInstruction(
            program_id=EVM_LOADER,
            data=data,
            keys=[
                # Storage account
                AccountMeta(storage_account, is_signer=False, is_writable=True),
                # Sysvar account
                AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                # Operator account
                AccountMeta(operator.public_key, is_signer=True, is_writable=True),
                # Treasury account
                AccountMeta(treasury_account, is_signer=False, is_writable=True),
                # Operator ether account
                AccountMeta(operator_eth_account, is_signer=False, is_writable=True),
                AccountMeta(SYS_PROGRAM_ID, is_signer=False, is_writable=True),
                # Neon EVM account
                AccountMeta(EVM_LOADER, is_signer=False, is_writable=False),

                # Program accounts
                AccountMeta(program_first_account, is_signer=False, is_writable=True),
                AccountMeta(program_second_account, is_signer=False, is_writable=True),
                # User account
                AccountMeta(user_account, is_signer=False, is_writable=True),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
            ]
        )
    )
    return send_transaction(solana_client, trx, operator)


#  We need test here two types of transaction
class TestStorageAccountAccess:
    def test_write_to_new_storage(self, operator_keypair, deploy_contract, user_account, treasury_pool, evm_loader):
        """
        Verify that evm save state in storage account and after finish finalize it
        """
        # 0. Deploy the contract
        # 1. Generate ABI to transaction
        # 2. Generate eth transaction
        # 3. Create storage account
        # 4. Execute Transaction with low count of step, check storage account
        # 5. Complete execution and check storage is finalized
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x1) + bytes.fromhex("%064x" % 0x1))
        eth_transaction = make_eth_transaction(user_account.solana_account,
                                               user_account.solana_account_address,
                                               deploy_contract.eth_address,
                                               user_account.ether_address,
                                               data)
        storage_account = create_storage_account(eth_transaction[1][:8].hex(), operator_keypair)
        instruction = eth_transaction[0] + eth_transaction[1] + eth_transaction[2]
        tx = make_PartialCallOrContinueFromRawEthereumTX_transaction(
            treasury_pool.buffer,
            1,
            instruction,
            operator_keypair,
            storage_account,
            treasury_pool.account,
            evm_loader.ether2program(eth_keys.PrivateKey(operator_keypair.secret_key[:32]).public_key.to_canonical_address())[0],
            deploy_contract.program,
            deploy_contract.code,
            user_account.solana_account_address,
        )
        assert tx["result"]['meta']['err'] is None

    def test_write_to_locked(self):
        """EVM can't write to locked storage account"""

    def test_write_to_finalized(self):
        """EVM can write to finalized storage account"""

    def test_two_trx_with_same_accounts(self):
        pass

    def test_cancel_trx(self):
        pass
