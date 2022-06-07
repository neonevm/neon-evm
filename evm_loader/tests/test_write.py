# Related issue: https://github.com/neonlabsorg/neon-evm/issues/261
# Test for the WriteHolder instruction.
# 1. Checks the operator can write to a holder account.
# 2. Checks the operator cannot write to a holder account with wrong seed.
# 3. Checks no one other can write to a holder account.

import pytest
from solana.rpc.core import RPCException
from solana.keypair import Keypair
from .solana_utils import (
    solana_cli,
    get_solana_balance,
    PublicKey,
    keccak_256,
    account_with_seed,
    solana_client,
    create_account_with_seed,
    TxOpts,
    Confirmed,
    AccountMeta,
    TransactionInstruction,
)
from .utils.instructions import TransactionWithComputeBudget
from .utils.constants import EVM_LOADER


test_data = b"Chancellor on brink of second bailout for banks"
path_to_solana = "solana"

holder_id = 0


def write_holder_layout(nonce, offset, data):
    return (
        bytes.fromhex("12")
        + nonce.to_bytes(8, byteorder="little")
        + offset.to_bytes(4, byteorder="little")
        + len(data).to_bytes(8, byteorder="little")
        + data
    )


def create_account(signer: Keypair) -> PublicKey:
    holder_id_bytes = holder_id.to_bytes((holder_id.bit_length() + 7) // 8, "big")
    seed = keccak_256(b"holder" + holder_id_bytes).hexdigest()[:32]
    account = account_with_seed(signer.public_key, seed, PublicKey(EVM_LOADER))
    if get_solana_balance(account) == 0:
        print("Creating account...")
        trx = TransactionWithComputeBudget()
        trx.add(
            create_account_with_seed(
                signer.public_key, signer.public_key, seed, 10 ** 9, 128 * 1024, PublicKey(EVM_LOADER)
            )
        )
        solana_client.send_transaction(
            trx, signer, opts=TxOpts(skip_confirmation=False, preflight_commitment=Confirmed)
        )
    print("Account to write:", account)
    print("Balance of account:", get_solana_balance(account))
    return account


def write_to_account(account, operator, signer, nonce, data) -> int:
    tx = TransactionWithComputeBudget()
    metas = [
        AccountMeta(pubkey=account, is_signer=False, is_writable=True),
        AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=False),
    ]
    tx.add(TransactionInstruction(program_id=EVM_LOADER, data=write_holder_layout(nonce, 0, data), keys=metas))
    opts = TxOpts(skip_confirmation=True, preflight_commitment=Confirmed)
    return solana_client.send_transaction(tx, signer, opts=opts)["id"]


@pytest.fixture(scope="class")
def attacker() -> Keypair:
    values = bytes([1] * 32)
    attacker = Keypair.from_secret_key(values)
    solana_cli().call("transfer" + " --allow-unfunded-recipient " + str(attacker.public_key) + " 1")
    print("Attacker:", attacker.public_key)
    print("Balance of attacker:", get_solana_balance(attacker.public_key))
    return attacker


@pytest.fixture(scope="class")
def account(operator_keypair) -> PublicKey:
    return create_account(operator_keypair)


class TestWriteAccount:
    def test_instruction_write_is_ok(self, account, operator_keypair):
        tx_id = write_to_account(account, operator_keypair, operator_keypair, holder_id, test_data)
        assert tx_id > 0

    def test_instruction_write_fails_wrong_seed(self, account, operator_keypair):
        with pytest.raises(
            RPCException,
            match="Transaction simulation failed: Error processing Instruction 2: invalid program argument",
        ):
            wrong_holder_id = 1000
            write_to_account(account, operator_keypair, operator_keypair, wrong_holder_id, test_data)

    def test_instruction_write_fails_wrong_operator(self, account, attacker):
        with pytest.raises(
            RPCException,
            match="Transaction simulation failed: Error processing Instruction 2: custom program error: 0x3",
        ):
            write_to_account(account, attacker, attacker, holder_id, test_data)
