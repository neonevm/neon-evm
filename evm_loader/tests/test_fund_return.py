import pytest
from solana.rpc.core import RPCException
from solana.keypair import Keypair
from solana.transaction import AccountMeta, TransactionInstruction
from solana_utils import sha256, solana_client, wait_confirm_transaction, get_solana_balance,\
    TransactionWithComputeBudget, create_account_with_seed, PublicKey, EVM_LOADER, send_transaction


def create_account(seed: str):
    bytes_seed = sha256(bytes(seed, 'utf8')).digest()
    new_acc = Keypair.from_seed(bytes_seed)
    trx = solana_client.request_airdrop(new_acc.public_key, 10 * 10 ** 9)
    wait_confirm_transaction(solana_client, trx['result'])
    return new_acc


def create_account_with_seed_from_acc(acc: Keypair, seed: str):
    storage = PublicKey(
        sha256(bytes(acc.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())

    if get_solana_balance(storage) == 0:
        trx = TransactionWithComputeBudget()
        trx.add(create_account_with_seed(acc.public_key, acc.public_key, seed, 10 ** 9, 128 * 1024,
                                         PublicKey(EVM_LOADER)))
        send_transaction(solana_client, trx, acc)

    return storage


def call_refund_tx(del_key: PublicKey, acc: Keypair, seed: str, signer: Keypair):
    trx = TransactionWithComputeBudget()
    trx.add(TransactionInstruction(
        program_id=EVM_LOADER,
        data=bytearray.fromhex("10") + bytes(seed, 'utf8'),
        keys=[
            AccountMeta(pubkey=del_key, is_signer=False, is_writable=True),
            AccountMeta(pubkey=acc.public_key, is_signer=(signer == acc), is_writable=True),
        ]))
    return send_transaction(solana_client, trx, signer)


class TestFundReturn:
    alice_acc: Keypair
    bob_acc: Keypair
    refundable_acc: PublicKey
    fail_acc: PublicKey
    refundable_seed = "refund"
    fail_seed = "fail"

    @classmethod
    def setup_class(cls):
        cls.alice_acc = create_account("alice")
        cls.bob_acc = create_account("bob")

        cls.refundable_acc = create_account_with_seed_from_acc(cls.alice_acc, cls.refundable_seed)

        cls.fail_acc = create_account_with_seed_from_acc(cls.alice_acc, cls.fail_seed)

    def test_creator_not_signer(self):
        err_msg = "expected signer"

        with pytest.raises(RPCException, match=err_msg):
            call_refund_tx(self.fail_acc, self.alice_acc, self.fail_seed, self.bob_acc)

    def test_error_on_wrong_creator(self):
        err_msg = "invalid account data for instruction"

        with pytest.raises(RPCException, match=err_msg):
            call_refund_tx(self.fail_acc, self.bob_acc, self.fail_seed, self.bob_acc)

    def test_error_on_wrong_seed(self):
        err_msg = "invalid account data for instruction"

        with pytest.raises(RPCException, match=err_msg):
            call_refund_tx(self.fail_acc, self.alice_acc, self.refundable_seed, self.alice_acc)

    def test_success_refund(self):
        pre_storage = get_solana_balance(self.refundable_acc)
        pre_acc = get_solana_balance(self.alice_acc.public_key)

        call_refund_tx(self.refundable_acc, self.alice_acc, self.refundable_seed, self.alice_acc)

        post_acc = get_solana_balance(self.alice_acc.public_key)

        assert pre_storage + pre_acc, post_acc + 5000
