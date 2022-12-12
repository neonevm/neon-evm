# Related issue: https://github.com/neonlabsorg/neon-evm/issues/261
# Test for the WriteHolder instruction.
# 1. Checks the operator can write to a holder account.
# 2. Checks the operator cannot write to a holder account with wrong seed.
# 3. Checks no one other can write to a holder account.

import pytest
from solana.rpc.core import RPCException
from solana.keypair import Keypair
from .solana_utils import (
    create_holder_account,
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
    Transaction,
    TransactionInstruction,
)
from .utils.instructions import write_holder_layout
from .utils.constants import EVM_LOADER


test_data = b"Chancellor on brink of second bailout for banks"
path_to_solana = "solana"

holder_id = 0


def create_account(signer: Keypair) -> PublicKey:
    holder_id_bytes = holder_id.to_bytes((holder_id.bit_length() + 7) // 8, "big")
    seed = keccak_256(b"holder" + holder_id_bytes).hexdigest()[:32]
    account = account_with_seed(signer.public_key, seed, PublicKey(EVM_LOADER))
    if get_solana_balance(account) == 0:
        print("Creating account...")
        trx = Transaction()
        trx.add(
            create_account_with_seed(
                signer.public_key, signer.public_key, seed, 10 ** 9, 128 * 1024, PublicKey(EVM_LOADER)
            ),
            create_holder_account(
                account, signer.public_key
            )
        )
        solana_client.send_transaction(
            trx, signer, opts=TxOpts(skip_confirmation=False, preflight_commitment=Confirmed)
        )
    print("Account to write:", account)
    print("Balance of account:", get_solana_balance(account))
    return account


def write_to_account(account, operator, signer, hash, data) -> int:
    tx = Transaction()
    metas = [
        AccountMeta(pubkey=account, is_signer=False, is_writable=True),
        AccountMeta(pubkey=operator.public_key, is_signer=True, is_writable=False),
    ]
    tx.add(TransactionInstruction(program_id=PublicKey(EVM_LOADER), data=write_holder_layout(hash, 0, data), keys=metas))
    opts = TxOpts(skip_confirmation=True, preflight_commitment=Confirmed)
    return solana_client.send_transaction(tx, signer, opts=opts).value


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
        tx_id = write_to_account(account, operator_keypair, operator_keypair, bytes(32), test_data)
        assert tx_id is not None

    def test_instruction_write_fails_wrong_operator(self, account, attacker):
        with pytest.raises(
            RPCException,
            match="Transaction simulation failed: Error processing Instruction 0: invalid account data for instruction",
        ):
            write_to_account(account, attacker, attacker, bytes(32), test_data)
