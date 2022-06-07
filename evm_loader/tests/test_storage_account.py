import base64
import pathlib

import pytest

from solana.keypair import Keypair

from eth_utils import abi
from .solana_utils import send_transaction, solana_client, get_transaction_count, neon_cli
from .utils.storage import create_storage_account
from .utils.contract import deploy_contract
from .utils.ethereum import make_eth_transaction
from .utils.instructions import make_PartialCallOrContinueFromRawEthereumTX, TransactionWithComputeBudget, \
    make_CancelWithNonce
from .utils.layouts import STORAGE_ACCOUNT_INFO_LAYOUT, FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT


@pytest.fixture(scope="module")
def deployed_contract(evm_loader: "EvmLoader", user_account: "Caller", operator_keypair: Keypair,
                      treasury_pool) -> "Contract":
    hello_world_contract_path = pathlib.Path(__file__).parent / "contracts" / "hello_world.binary"
    return deploy_contract(operator_keypair, user_account, hello_world_contract_path, evm_loader, treasury_pool)


#  We need test here two types of transaction
class TestStorageAccountAccess:
    def test_write_to_new_storage_and_finalize(self, operator_keypair, deployed_contract, user_account, treasury_pool,
                                               evm_loader):
        """
        Verify that evm save state in storage account and after finish finalize it
        """
        func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
        data = (func_name + bytes.fromhex("%064x" % 0x10) + bytes.fromhex("%064x" % 0x10))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.ether_address
        )
        storage_account = create_storage_account(operator_keypair)
        instruction = eth_transaction[0] + eth_transaction[1] + eth_transaction[2]

        print("EMULATE")
        print(neon_cli().emulate(evm_loader.loader_id, f"{user_account.ether_address.hex()} {deployed_contract.eth_address.hex()} {data.hex()}"))

        # trx = TransactionWithComputeBudget()
        # trx.add(
        #     make_PartialCallOrContinueFromRawEthereumTX(
        #         instruction,
        #         operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
        #         [
        #             deployed_contract.solana_address,
        #             deployed_contract.code_solana_address,
        #             user_account.solana_account_address,
        #         ]
        #     )
        # )
        # receipt = send_transaction(solana_client, trx, operator_keypair)
        # assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        # account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        # parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        # assert parsed_data.tag == 30
        # assert parsed_data.caller == user_account.ether_address
        #
        # # finish transaction and check storage is finalized
        # trx = TransactionWithComputeBudget()
        # trx.add(
        #     make_PartialCallOrContinueFromRawEthereumTX(
        #         instruction,
        #         operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1000,
        #         [
        #             deployed_contract.solana_address,
        #             deployed_contract.code_solana_address,
        #             user_account.solana_account_address,
        #         ]
        #     )
        # )
        # send_transaction(solana_client, trx, operator_keypair)
        # account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        # parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        # assert parsed_data.tag == 5

    def test_write_to_locked(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can't write to locked storage account"""
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        data = (func_name + bytes.fromhex("%064x" % 0x10))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.ether_address
        )
        storage_account = create_storage_account(operator_keypair)
        instruction = eth_transaction[0] + eth_transaction[1] + eth_transaction[2]
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
                [
                    deployed_contract.solana_address,
                    deployed_contract.code_solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == 30

    def test_write_to_finalized(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can write to finalized storage account"""
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        data = (func_name + bytes.fromhex("%064x" % 0x10))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.ether_address
        )
        storage_account = create_storage_account(operator_keypair)
        instruction = eth_transaction[0] + eth_transaction[1] + eth_transaction[2]
        for i in range(2):
            trx = TransactionWithComputeBudget()
            trx.add(
                make_PartialCallOrContinueFromRawEthereumTX(
                    instruction,
                    operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1000,
                    [
                        deployed_contract.solana_address,
                        deployed_contract.code_solana_address,
                        user_account.solana_account_address,
                    ]
                )
            )
            receipt = send_transaction(solana_client, trx, operator_keypair)
            account_data = base64.b64decode(
                solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
            parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
            assert parsed_data.tag == 5

    def test_cancel_trx(self, operator_keypair, deployed_contract, user_account, treasury_pool, evm_loader):
        """EVM can cancel transaction and finalize storage account"""
        func_name = abi.function_signature_to_4byte_selector('update_storage(uint256)')
        data = (func_name + bytes.fromhex("%064x" % 0x10))
        eth_transaction = make_eth_transaction(
            deployed_contract.eth_address,
            data,
            user_account.solana_account,
            user_account.solana_account_address,
            user_account.ether_address
        )
        storage_account = create_storage_account(operator_keypair)
        instruction = eth_transaction[0] + eth_transaction[1] + eth_transaction[2]
        trx = TransactionWithComputeBudget()
        trx.add(
            make_PartialCallOrContinueFromRawEthereumTX(
                instruction,
                operator_keypair, evm_loader, storage_account, treasury_pool.account, treasury_pool.buffer, 1,
                [
                    deployed_contract.solana_address,
                    deployed_contract.code_solana_address,
                    user_account.solana_account_address,
                ]
            )
        )
        receipt = send_transaction(solana_client, trx, operator_keypair)
        assert "success" in receipt["result"]["meta"]["logMessages"][-1]
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == 30
        user_nonce = get_transaction_count(solana_client, user_account.solana_account_address)
        trx = TransactionWithComputeBudget()
        trx.add(
            make_CancelWithNonce(storage_account, operator_keypair, user_nonce,
                                 [
                                     user_account.solana_account_address,
                                     deployed_contract.solana_address,
                                     deployed_contract.code_solana_address
                                 ]
                                 )
        )
        send_transaction(solana_client, trx, operator_keypair)
        account_data = base64.b64decode(solana_client.get_account_info(storage_account)["result"]["value"]["data"][0])
        parsed_data = FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT.parse(account_data)
        assert parsed_data.tag == 5
        assert user_nonce < get_transaction_count(solana_client, user_account.solana_account_address)
