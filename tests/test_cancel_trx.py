from solana.transaction import Transaction

from .solana_utils import send_transaction, solana_client, \
    send_transaction_step_from_instruction
from .utils.constants import TAG_FINALIZED_STATE, TAG_STATE
from .utils.contract import make_contract_call_trx
from .utils.storage import create_holder
from .utils.instructions import make_Cancel
from .utils.layouts import FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, STORAGE_ACCOUNT_INFO_LAYOUT
from .utils.transaction_checks import check_holder_account_tag


#  We need test here two types of transaction
class TestCancelTrx:

    def test_cancel_trx(self, operator_keypair, rw_lock_contract, user_account, treasury_pool, evm_loader):
        """EVM can cancel transaction and finalize storage account"""
        signed_tx = make_contract_call_trx(user_account, rw_lock_contract, "unchange_storage(uint8,uint8)", [1, 1])

        storage_account = create_holder(operator_keypair)
        trx = send_transaction_step_from_instruction(operator_keypair, evm_loader, treasury_pool, storage_account,
                                                     signed_tx,
                                                     [rw_lock_contract.solana_address,
                                                      rw_lock_contract.balance_account_address,
                                                      user_account.balance_account_address],
                                                     1, operator_keypair)

        receipt = solana_client.get_transaction(trx.value)
        assert receipt.value.transaction.meta.err is None
        check_holder_account_tag(storage_account, STORAGE_ACCOUNT_INFO_LAYOUT, TAG_STATE)

        user_nonce = evm_loader.get_neon_nonce(user_account.eth_address)
        trx = Transaction()
        trx.add(
            make_Cancel(evm_loader, storage_account, operator_keypair, signed_tx.hash,
                        [rw_lock_contract.solana_address,
                        rw_lock_contract.balance_account_address,
                        user_account.balance_account_address])
        )
        send_transaction(solana_client, trx, operator_keypair)
        check_holder_account_tag(storage_account, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT, TAG_FINALIZED_STATE)
        assert user_nonce < evm_loader.get_neon_nonce(user_account.eth_address)
