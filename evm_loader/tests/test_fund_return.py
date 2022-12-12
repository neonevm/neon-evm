import pytest
import solders
from solana.rpc.core import RPCException
from solana.keypair import Keypair
from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from .solana_utils import account_with_seed, create_holder_account, sha256, solana_client, wait_confirm_transaction, get_solana_balance,\
     create_account_with_seed, PublicKey, send_transaction
from .utils.constants import EVM_LOADER


def create_account(seed: str):
    bytes_seed = sha256(bytes(seed, 'utf8')).digest()
    new_acc = Keypair.from_seed(bytes_seed)
    trx = solana_client.request_airdrop(new_acc.public_key, 10 * 10 ** 9)
    wait_confirm_transaction(solana_client, trx.value)
    return new_acc


def create_holder(acc: Keypair, seed: str):
    account = account_with_seed(acc.public_key, seed, PublicKey(EVM_LOADER))

    if get_solana_balance(account) == 0:
        trx = Transaction()
        trx.add(
            create_account_with_seed(acc.public_key, acc.public_key, seed, 10 ** 9, 128 * 1024, PublicKey(EVM_LOADER)),
            create_holder_account(account, acc.public_key)
        )
        send_transaction(solana_client, trx, acc)

    return account


def delete_holder(del_key: PublicKey, acc: Keypair, signer: Keypair):
    trx = Transaction()

    trx.add(TransactionInstruction(
        program_id=PublicKey(EVM_LOADER),
        data=bytes.fromhex("25"),
        keys=[
            AccountMeta(pubkey=del_key, is_signer=False, is_writable=True),
            AccountMeta(pubkey=acc.public_key, is_signer=(signer == acc), is_writable=True),
        ]))
    return send_transaction(solana_client, trx, signer)


class TestFundReturn:
    alice: Keypair
    bob: Keypair
    alice_account: PublicKey
    bob_account: PublicKey


    @classmethod
    def setup_class(cls):
        cls.alice = create_account("alice")
        cls.bob = create_account("bob")

        cls.alice_account = create_holder(cls.alice, "1")
        cls.bob_account = create_holder(cls.bob, "2")

    def test_error_on_wrong_creator(self):
        err_msg = "invalid account data for instruction"

        with pytest.raises(RPCException, match=err_msg):
            delete_holder(self.alice_account, self.bob, self.bob)

    def test_success_refund(self):
        pre_storage = get_solana_balance(self.alice_account)
        pre_acc = get_solana_balance(self.alice.public_key)

        delete_holder(self.alice_account, self.alice, self.alice)

        post_acc = get_solana_balance(self.alice.public_key)

        assert pre_storage + pre_acc, post_acc + 5000
