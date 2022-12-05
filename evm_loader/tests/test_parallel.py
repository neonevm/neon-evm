from typing import Any
from unittest import TestCase

from _pytest.fixtures import fixture
from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Finalized
from solana.rpc.core import RPCException

from .solana_utils import EvmLoader, send_transaction, solana_client, get_account_data, make_new_user, deposit_neon, \
    cancel_transaction
from .utils.contract import write_transaction_to_holder_account, deploy_contract_step, make_deployment_transaction
from .utils.ethereum import create_contract_address, make_eth_transaction
from .utils.instructions import TransactionWithComputeBudget, make_ExecuteTrxFromInstruction
from .utils.layouts import ACCOUNT_INFO_LAYOUT
from .utils.storage import create_holder
from .utils.types import Caller, TreasuryPool

EVM_STEPS_COUNT = 0xFFFFFFFF_FFFFFFFF
ONE_TOKEN = 10 ** 9
BIG_CONTRACT_FILENAME = "ERC20ForSplFactory.binary"
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
            [2, ACCOUNT_INFO_LAYOUT.sizeof()],
            [3, MAX_PERMITTED_DATA_INCREASE],
            [4, MAX_PERMITTED_DATA_INCREASE * 2],
        ]

        for case in cases:
            iterations = case[0]
            expected_length = case[1]
            with self.subTest(iterations=iterations, expected_length=expected_length):
                self.create_same_accounts_subtest(iterations, expected_length)

    def create_same_accounts_subtest(self, iterations: int, expected_length: int):
        deposit_neon(self.evm_loader, self.operator_keypair, self.user_account.eth_address, ONE_TOKEN)
        deposit_neon(self.evm_loader, self.operator_keypair, self.second_account.eth_address, ONE_TOKEN)

        contract = create_contract_address(self.user_account, self.evm_loader)
        holder_acc = create_holder(self.operator_keypair)
        deployment_tx = make_deployment_transaction(self.user_account, BIG_CONTRACT_FILENAME)
        write_transaction_to_holder_account(deployment_tx, holder_acc, self.operator_keypair)

        # First N iterations
        for i in range(iterations):
            deployment_receipt = deploy_contract_step(
                EVM_STEPS_COUNT,
                self.treasury_pool,
                holder_acc,
                self.operator_keypair,
                self.evm_loader,
                contract,
                self.user_account,
            )

            assert not ParallelTransactionsTest.check_iteration_deployed(deployment_receipt)

        # Transferring to the same account in order to break deployment
        ParallelTransactionsTest.transfer(
            self.second_account,
            contract.eth_address,
            ONE_TOKEN,
            contract.solana_address,
            self.evm_loader,
            self.operator_keypair,
            self.treasury_pool,
        )

        # Trying to finish deployment (expected to fail)
        try:
            deploy_contract_step(
                EVM_STEPS_COUNT,
                self.treasury_pool,
                holder_acc,
                self.operator_keypair,
                self.evm_loader,
                contract,
                self.user_account,
            )

            assert False, 'Deployment expected to fail'
        except RPCException as e:
            ParallelTransactionsTest.check_account_initialized_in_another_trx_exception(e, contract.solana_address)

        # Cancel deployment transaction:
        cancel_transaction(
            deployment_tx.hash,
            holder_acc,
            self.operator_keypair,
            [contract.solana_address, self.user_account.solana_account_address],
        )

        data = get_account_data(solana_client, contract.solana_address, expected_length)
        assert len(data) == expected_length

        account = ACCOUNT_INFO_LAYOUT.parse(data)
        assert account.code_size == 0
        balance = int.from_bytes(account.balance, byteorder="little")
        assert balance == ONE_TOKEN

    @staticmethod
    def check_iteration_deployed(receipt: Any) -> bool:
        if receipt["meta"]["err"]:
            raise AssertionError(f"Can't deploy contract: {receipt['meta']['err']}")
        for log in receipt["meta"]["logMessages"]:
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
        dst_solana_addr: PublicKey,
        evm_loader: EvmLoader,
        operator_keypair: Keypair,
        treasury_pool: TreasuryPool,
    ):
        message = make_eth_transaction(
            dst_addr,
            bytes(),
            src_account.solana_account,
            src_account.solana_account_address,
            value,
        ).rawTransaction

        trx = TransactionWithComputeBudget()
        trx.add(
            make_ExecuteTrxFromInstruction(
                operator_keypair,
                evm_loader,
                treasury_pool.account,
                treasury_pool.buffer,
                message,
                [dst_solana_addr, src_account.solana_account_address],
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair, Finalized)
        print(type(receipt))
        print("Transfer receipt:", receipt)
        assert receipt.value[0].err is None

        return receipt

    @staticmethod
    def check_account_initialized_in_another_trx_exception(exception: RPCException, solana_address: PublicKey):
        error = exception.args[0]
        print("error:", error)
        assert error['code'] == -32002
        assert 'instruction requires an uninitialized account' in error['message']

        for log in error['data']['logs']:
            if f'Blocked nonexistent account {solana_address} was created/initialized outside' in log:
                return

        assert False, "Search string not found in Solana logs"
