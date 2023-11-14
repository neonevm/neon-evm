from typing import Any
from unittest import TestCase

from _pytest.fixtures import fixture
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.core import RPCException

from .solana_utils import EvmLoader, solana_client, get_solana_account_data, make_new_user, deposit_neon, \
    cancel_transaction, send_transaction_step_from_account, execute_trx_from_instruction
from .utils.contract import write_transaction_to_holder_account, make_deployment_transaction
from .utils.ethereum import create_contract_address, make_eth_transaction
from .utils.layouts import CONTRACT_ACCOUNT_LAYOUT
from .utils.storage import create_holder
from .utils.types import Caller, TreasuryPool

EVM_STEPS_COUNT = 0xFFFFFFFF_FFFFFFFF
ONE_TOKEN = 10 ** 9
BIG_CONTRACT_FILENAME = "BigContract.binary"
MAX_PERMITTED_DATA_INCREASE = 10240


class ParallelTransactionsTest(TestCase):
    @fixture(autouse=True)
    def prepare_fixture(
            self,
            user_account: Caller,
            evm_loader: EvmLoader,
            operator_keypair: Keypair,
            treasury_pool: TreasuryPool,
    ):
        self.user_account = user_account
        self.evm_loader = evm_loader
        self.operator_keypair = operator_keypair
        self.treasury_pool = treasury_pool
        self.second_account = make_new_user(evm_loader)

    def test_create_same_accounts(self):
        cases = [
            [2],
            [3],
            [4],
        ]

        for case in cases:
            iterations = case[0]
            with self.subTest(iterations=iterations):
                self.create_same_accounts_subtest(iterations)

    def create_same_accounts_subtest(self, iterations: int):
        deposit_neon(self.evm_loader, self.operator_keypair, self.user_account.eth_address, ONE_TOKEN)
        deposit_neon(self.evm_loader, self.operator_keypair, self.second_account.eth_address, ONE_TOKEN)

        contract = create_contract_address(self.user_account, self.evm_loader)
        holder_acc = create_holder(self.operator_keypair)
        deployment_tx = make_deployment_transaction(self.user_account, BIG_CONTRACT_FILENAME)
        write_transaction_to_holder_account(deployment_tx, holder_acc, self.operator_keypair)

        # First N iterations
        for i in range(iterations):
            deployment_receipt = send_transaction_step_from_account(self.operator_keypair,
                                                                    self.evm_loader,
                                                                    self.treasury_pool,
                                                                    holder_acc,
                                                                    [contract.balance_account_address,
                                                                     contract.solana_address,
                                                                     self.user_account.balance_account_address,
                                                                     self.user_account.solana_account_address],
                                                                    EVM_STEPS_COUNT,
                                                                    self.operator_keypair)

            assert not ParallelTransactionsTest.check_iteration_deployed(deployment_receipt)

        # Transferring to the same account in order to break deployment
        ParallelTransactionsTest.transfer(
            self.second_account,
            contract.eth_address,
            ONE_TOKEN,
            self.evm_loader,
            self.operator_keypair,
            self.treasury_pool,
        )

        # Trying to finish deployment (expected to fail)
        try:
            send_transaction_step_from_account(self.operator_keypair,
                                               self.evm_loader,
                                               self.treasury_pool,
                                               holder_acc,
                                               [contract.balance_account_address,
                                                contract.solana_address,
                                                self.user_account.balance_account_address,
                                                self.user_account.solana_account_address],
                                               EVM_STEPS_COUNT,
                                               self.operator_keypair)

            assert False, 'Deployment expected to fail'
        except RPCException as e:
            ParallelTransactionsTest.check_account_initialized_in_another_trx_exception(e, contract.balance_account_address)

        # Cancel deployment transaction:
        cancel_transaction(
            self.evm_loader,
            deployment_tx.hash,
            holder_acc,
            self.operator_keypair,
            [contract.balance_account_address,
             contract.solana_address,
             self.user_account.balance_account_address,
             self.user_account.solana_account_address],
        )

    @staticmethod
    def check_iteration_deployed(receipt: Any) -> bool:
        if receipt.value.transaction.meta.err:
            raise AssertionError(f"Can't deploy contract: {receipt.value.transaction.meta.err}")

        for log in receipt.value.transaction.meta.log_messages:
            if "exit_status" in log:
                return True
            if "ExitError" in log:
                raise AssertionError(f"EVM Return error in logs: {receipt}")
        return False

    @staticmethod
    def transfer(
            src_account: Caller,
            dst_addr: bytes,
            value: int,
            evm_loader: EvmLoader,
            operator_keypair: Keypair,
            treasury_pool: TreasuryPool,
    ):
        message = make_eth_transaction(
            dst_addr,
            bytes(),
            src_account,
            value,
        )

        dst_solana_account, _ = evm_loader.ether2program(dst_addr)
        dst_balance_account = evm_loader.ether2balance(dst_addr)

        trx = execute_trx_from_instruction(operator_keypair, evm_loader, treasury_pool.account, treasury_pool.buffer,
                                           message,
                                           [dst_balance_account,
                                            PublicKey(dst_solana_account),
                                            src_account.balance_account_address],
                                           operator_keypair)
        receipt = solana_client.get_transaction(trx.value)
        print("Transfer receipt:", receipt)
        assert receipt.value.transaction.meta.err is None

        return receipt

    @staticmethod
    def check_account_initialized_in_another_trx_exception(exception: RPCException, solana_address: PublicKey):
        error = exception.args[0]
        print("error:", error)
        
        for log in error.data.logs:
            if f'Account {solana_address} - was empty, created by another transaction' in log:
                return

        assert False, "Search string not found in Solana logs"
