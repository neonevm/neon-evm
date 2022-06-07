import json
import pathlib


import pytest

from solana.keypair import Keypair
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed
from eth_keys import keys as eth_keys

from .solana_utils import ETH_TOKEN_MINT_ID, EvmLoader, OperatorAccount, get_solana_balance, create_treasury_pool_address, solana_client, \
    wait_confirm_transaction, get_associated_token_address, make_new_user
from .utils.types import TreasuryPool, Caller


def pytest_addoption(parser):
    parser.addoption(
        "--operator-key", action="store", default="~/.config/solana/id.json", help="Path to operator keypair"
    )


@pytest.fixture(scope="session")
def operator_keypair(request) -> Keypair:
    """
    Initialized solana keypair with balance. Get private key from cli or ~/.config/solana/id.json
    """
    with open(pathlib.Path(request.config.getoption("--operator-key")).expanduser(), "r") as key:
        account = Keypair(json.load(key)[:32])
    return account


@pytest.fixture(scope="session")
def evm_loader(request) -> EvmLoader:
    wallet = OperatorAccount(pathlib.Path(request.config.getoption("--operator-key")).expanduser().as_posix())
    loader = EvmLoader(wallet)
    return loader


@pytest.fixture(scope="module")
def treasury_pool(evm_loader) -> TreasuryPool:
    index = 2
    address = create_treasury_pool_address(index)
    index_buf = index.to_bytes(4, 'little')
    return TreasuryPool(index, address, index_buf)


@pytest.fixture(scope="module")
def user_account(evm_loader) -> Caller:
    return make_new_user(evm_loader)
