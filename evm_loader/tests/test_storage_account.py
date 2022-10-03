import base64

import pytest

from solana.keypair import Keypair
from solana.rpc.core import RPCException
from solana.transaction import Transaction

from eth_utils import abi
from .solana_utils import send_transaction, solana_client, get_transaction_count, make_new_user
from .utils.constants import TAG_STATE, TAG_FINALIZED_STATE
from .utils.storage import create_holder
from .utils.contract import deploy_contract
from .utils.ethereum import make_eth_transaction
from .utils.instructions import make_PartialCallOrContinueFromRawEthereumTX, TransactionWithComputeBudget, \
    make_Cancel
from .utils.layouts import STORAGE_ACCOUNT_INFO_LAYOUT, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT


@pytest.fixture(scope="function")
def deployed_contract(evm_loader: "EvmLoader", user_account: "Caller", operator_keypair: Keypair,
                      treasury_pool) -> "Contract":
    return deploy_contract(operator_keypair, user_account, "rw_lock.binary", evm_loader, treasury_pool)


#  We need test here two types of transaction
class TestStorageAccountAccess:
    def test_write_to_new_storage_and_finalize(self, operator_keypair, deployed_contract, user_account, treasury_pool,
                                               evm_loader):
        """
        Verify that evm save state in storage account and after finish finalize it
        """
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x01) + bytes.fromhex("%064x" % 0x01))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.eth_address
        )
        storage_account = create_holder(operator_keypair)
        instruction = eth_transaction.rawTransaction

        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 10,
                [
                    deployed_contract.solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == TAG_STATE
        assert parsed_data.caller == user_account.eth_address
        #
        # # finish transaction and check storage is finalized
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1000,
                [
                    deployed_contract.solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == TAG_FINALIZED_STATE
        assert "exit_status=0x12" in "\n".join(receipt["result"]["meta"]["logMessages"])

    def test_write_to_locked(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can't write to locked storage account"""
        storage_account = create_holder(operator_keypair)
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x01) + bytes.fromhex("%064x" % 0x01))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.eth_address
        )
        instruction = eth_transaction.rawTransaction
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
                [
                    deployed_contract.solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == TAG_STATE
        user2 = make_new_user(evm_loader)
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user2.solana_account,
            user2.solana_account_address,
            user2.eth_address
        )
        instruction = eth_transaction.rawTransaction
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 100,
                [
                    deployed_contract.solana_address,
                    user2.solana_account_address,
                ]
            )
        )
        with pytest.raises(RPCException, match="invalid account data for instruction") as e:
            send_transaction(solana_client, trx, operator_keypair)

    def test_write_to_finalized(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can write to finalized storage account"""
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        storage_account = create_holder(operator_keypair)
        print("TEST ACCOUNTS")
        print(deployed_contract.eth_address.hex(), user_account.eth_address.hex())
        for i in range(1, 3):
            data = (func_name + bytes.fromhex("%064x" % i) + bytes.fromhex("%064x" % 0x01))
            eth_transaction = make_eth_transaction(
                deployed_contract.eth_address,
                data,
                user_account.solana_account,
                user_account.solana_account_address,
                user_account.eth_address
            )
            instruction = eth_transaction.rawTransaction

            trx = TransactionWithComputeBudget()
            trx.add(
                make_PartialCallOrContinueFromRawEthereumTX(
                    instruction,
                    operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1000,
                    [
                        deployed_contract.solana_address,
                        user_account.solana_account_address,
                    ]
                )
            )
            send_transaction(solana_client, trx, operator_keypair)
            account_data = base64.b64decode(
                solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
            parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
            assert parsed_data.tag == TAG_FINALIZED_STATE

    def test_cancel_trx(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can cancel transaction and finalize storage account"""
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x01) + bytes.fromhex("%064x" % 0x01))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.eth_address
        )
        storage_account = create_holder(operator_keypair)
        instruction = eth_transaction.rawTransaction
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
                [
                    deployed_contract.solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == TAG_STATE
        user_nonce = get_transaction_count(solana_client, user_account.solana_account_address)
        trx = Transaction()
        trx.add(
            make_Cancel(storage_account, operator_keypair, eth_transaction.hash,
                                 [
                                     deployed_contract.solana_address,
                                     user_account.solana_account_address,
                                 ]
                                 )
        )
        send_transaction(solana_client, trx, operator_keypair)
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == TAG_FINALIZED_STATE
        assert user_nonce < get_transaction_count(solana_client, user_account.solana_account_address)
