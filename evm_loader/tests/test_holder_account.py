from hashlib import sha256
from random import randrange

import pytest
import solana
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
from solana.transaction import Transaction

from . import solana_utils
from .solana_utils import solana_client, make_new_user, write_transaction_to_holder_account
from .test_fund_return import delete_holder
from .utils.constants import EVM_LOADER
from .utils.contract import make_deployment_transaction
from .utils.ethereum import make_eth_transaction
from .utils.storage import create_holder


class TestHolderAccount:

    def test_create_holder_account(self, operator_keypair):
        holder_acc = create_holder(operator_keypair)
        info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
        assert info.value is not None, "Holder account is not created"
        assert info.value.lamports == 1000000000, "Account balance is not correct"

    def test_create_the_same_holder_account_by_another_user(self, operator_keypair, user_account):
        seed = str(randrange(1000000))
        storage = PublicKey(
            sha256(bytes(operator_keypair.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())
        create_holder(operator_keypair, seed=seed, storage=storage)

        trx = Transaction()
        trx.add(
            solana_utils.create_account_with_seed(user_account.solana_account.public_key,
                                                  user_account.solana_account.public_key, seed, 10 ** 9, 128 * 1024),
            solana_utils.create_holder_account(storage, user_account.solana_account.public_key)
        )

        with pytest.raises(solana.rpc.core.RPCException, match='already initialized'):
            solana_utils.send_transaction(solana_client, trx, user_account.solana_account)

    def test_write_tx_to_holder(self, operator_keypair, user_account, evm_loader):
        recipient = make_new_user(evm_loader)
        holder_acc = create_holder(operator_keypair)
        signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                         user_account.solana_account_address, 10)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
        assert signed_tx.rawTransaction == info.value.data[65:65 + len(signed_tx.rawTransaction)], \
            "Account data is not correct"

    def test_write_tx_to_holder_in_parts(self, operator_keypair, user_account):
        holder_acc = create_holder(operator_keypair)
        contract_filename = "ERC20ForSplFactory.binary"

        signed_tx = make_deployment_transaction(user_account, contract_filename)
        write_transaction_to_holder_account(signed_tx, holder_acc, operator_keypair)
        info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
        assert signed_tx.rawTransaction == info.value.data[65:65 + len(signed_tx.rawTransaction)], \
            "Account data is not correct"

    def test_write_tx_to_holder_by_no_owner(self, operator_keypair, user_account, evm_loader):
        holder_acc = create_holder(operator_keypair)
        recipient = make_new_user(evm_loader)

        signed_tx = make_eth_transaction(recipient.eth_address, None, user_account.solana_account,
                                         user_account.solana_account_address, 10)
        with pytest.raises(solana.rpc.core.RPCException, match="invalid account data for instruction"):
            write_transaction_to_holder_account(signed_tx, holder_acc, user_account.solana_account)

    def test_delete_holder(self, operator_keypair):
        holder_acc = create_holder(operator_keypair)
        delete_holder(holder_acc, operator_keypair, operator_keypair)
        info = solana_client.get_account_info(holder_acc, commitment=Confirmed)
        assert info.value is None, "Holder account isn't deleted"

    def test_delete_holder_by_no_owner(self, operator_keypair, user_account):
        holder_acc = create_holder(operator_keypair)
        with pytest.raises(solana.rpc.core.RPCException, match="invalid account data for instruction"):
            delete_holder(holder_acc, user_account.solana_account, user_account.solana_account)
