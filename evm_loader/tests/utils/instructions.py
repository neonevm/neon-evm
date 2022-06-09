import typing as tp

from solana.publickey import PublicKey
from solana.keypair import Keypair
from solana.system_program import SYS_PROGRAM_ID
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from eth_keys import keys as eth_keys

from .constants import EVM_LOADER, SYSTEM_ADDRESS, SYS_INSTRUCT_ADDRESS, INCINERATOR_ADDRESS
from .layouts import CREATE_ACCOUNT_LAYOUT


DEFAULT_UNITS = 500 * 1000
DEFAULT_HEAP_FRAME = 256 * 1024
DEFAULT_ADDITIONAL_FEE = 0
COMPUTE_BUDGET_ID: PublicKey = PublicKey("ComputeBudget111111111111111111111111111111")


class ComputeBudget:
    @staticmethod
    def request_units(units, additional_fee):
        return TransactionInstruction(
            program_id=COMPUTE_BUDGET_ID,
            keys=[],
            data=bytes.fromhex("00") + units.to_bytes(4, "little") + additional_fee.to_bytes(4, "little")
        )

    @staticmethod
    def request_heap_frame(heap_frame):
        return TransactionInstruction(
            program_id=COMPUTE_BUDGET_ID,
            keys=[],
            data=bytes.fromhex("01") + heap_frame.to_bytes(4, "little")
        )


class TransactionWithComputeBudget(Transaction):
    def __init__(self,
                 units=DEFAULT_UNITS,
                 additional_fee=DEFAULT_ADDITIONAL_FEE,
                 heap_frame=DEFAULT_HEAP_FRAME,
                 *args, **kwargs):
        super().__init__(*args, **kwargs)
        if units:
            self.instructions.append(ComputeBudget.request_units(units, additional_fee))
        if heap_frame:
            self.instructions.append(ComputeBudget.request_heap_frame(heap_frame))


def make_CreateAccountV02(operator: Keypair, contract_address: PublicKey, contract_eth_address: bytes, nonce: int,
                          code_address: PublicKey = None):
    d = bytes.fromhex('18') + CREATE_ACCOUNT_LAYOUT.build(
        dict(ether=contract_eth_address, nonce=nonce))

    accounts = [
        AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=False),
        AccountMeta(pubkey=PublicKey(SYSTEM_ADDRESS), is_signer=False, is_writable=False),
        AccountMeta(pubkey=contract_address, is_signer=False, is_writable=True),
    ]

    if code_address:
        accounts.append(
            AccountMeta(pubkey=code_address, is_signer=False, is_writable=True),
        )

    return TransactionInstruction(
            program_id=EVM_LOADER,
            data=d,
            keys=accounts)


def write_holder_layout(nonce, offset, data):
    return (bytes.fromhex('12') +
            nonce.to_bytes(8, byteorder='little') +
            offset.to_bytes(4, byteorder='little') +
            len(data).to_bytes(8, byteorder='little') +
            data)


def make_WriteHolder(operator: PublicKey, holder_account: PublicKey, holder_id: int, offset: int, payload: bytes):
    d = write_holder_layout(holder_id, offset, payload)

    return TransactionInstruction(
                program_id=EVM_LOADER,
                data=d,
                keys=[
                    AccountMeta(pubkey=holder_account, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=operator, is_signer=True, is_writable=False),
                ])


def make_ExecuteTrxFromAccountDataIterativeOrContinue(
        operator: Keypair,
        evm_loader: "EvmLoader",
        holder_address: PublicKey,
        storage_address: PublicKey,
        treasury_address: PublicKey,
        treasury_buffer: bytes,
        step_count: int,
        additional_accounts: tp.List[PublicKey]):
    code = 14
    d = code.to_bytes(1, "little") + treasury_buffer + step_count.to_bytes(8, byteorder="little")
    operator_ether = eth_keys.PrivateKey(operator.secret_key[:32]).public_key.to_canonical_address()
    print("MAKE INSTR--------")
    print("Holder ", holder_address)
    print("Storage ", storage_address)
    print("Operator ", operator.public_key)
    print("Treasury ", treasury_address)
    print("Operator eth ", evm_loader.ether2program(operator_ether)[0])
    print("SYS ", SYS_PROGRAM_ID)
    print("EVM LOADER ", EVM_LOADER")

    accounts = [
                AccountMeta(pubkey=holder_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=storage_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=True),
                AccountMeta(pubkey=treasury_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=evm_loader.ether2program(operator_ether)[0], is_signer=False, is_writable=True),
                AccountMeta(SYS_PROGRAM_ID, is_signer=False, is_writable=True),
                # Neon EVM account
                AccountMeta(EVM_LOADER, is_signer=False, is_writable=False),
            ]
    for acc in additional_accounts:
        print("Additional acc ", acc)
        accounts.append(AccountMeta(acc, is_signer=False, is_writable=True),)

    return TransactionInstruction(
            program_id=EVM_LOADER,
            data=d,
            keys=accounts
        )


def make_PartialCallOrContinueFromRawEthereumTX(
        instruction: bytes,
        operator: Keypair,
        evm_loader: "EvmLoader",
        storage_address: PublicKey,
        treasury_address: PublicKey,
        treasury_buffer: bytes,
        step_count: int,
        additional_accounts: tp.List[PublicKey]):
    code = 13
    d = code.to_bytes(1, "little") + treasury_buffer + step_count.to_bytes(8, byteorder="little") + instruction
    operator_ether = eth_keys.PrivateKey(operator.secret_key[:32]).public_key.to_canonical_address()

    accounts = [
                AccountMeta(pubkey=storage_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=SYS_INSTRUCT_ADDRESS, is_signer=False, is_writable=True),
                AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=True),
                AccountMeta(pubkey=treasury_address, is_signer=False, is_writable=True),
                AccountMeta(pubkey=evm_loader.ether2program(operator_ether)[0], is_signer=False, is_writable=True),
                AccountMeta(SYS_PROGRAM_ID, is_signer=False, is_writable=True),
                # Neon EVM account
                AccountMeta(EVM_LOADER, is_signer=False, is_writable=False),
            ]
    for acc in additional_accounts:
        accounts.append(AccountMeta(acc, is_signer=False, is_writable=True),)

    return TransactionInstruction(
            program_id=EVM_LOADER,
            data=d,
            keys=accounts
        )


def make_CancelWithNonce(storage_address: PublicKey, operator: Keypair, nonce: int, additional_accounts: tp.List[PublicKey]):
    code = 21
    d = code.to_bytes(1, "little") + nonce.to_bytes(8, byteorder="little")

    accounts = [
        AccountMeta(pubkey=storage_address, is_signer=False, is_writable=True),
        AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=True),
        AccountMeta(pubkey=PublicKey(INCINERATOR_ADDRESS), is_signer=False, is_writable=True),
    ]

    for acc in additional_accounts:
        accounts.append(AccountMeta(acc, is_signer=False, is_writable=True),)

    return TransactionInstruction(
        program_id=EVM_LOADER,
        data=d,
        keys=accounts
    )
