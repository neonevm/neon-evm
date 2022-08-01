import os
import json
import pathlib

import pytest

from solana.keypair import Keypair
from eth_keys import keys as eth_keys

from .solana_utils import EvmLoader, OperatorAccount, create_treasury_pool_address, make_new_user, get_solana_balance
from .utils.types import TreasuryPool, Caller


def pytest_addoption(parser):
    parser.addoption(
        "--operator-key", action="store", default="~/.config/solana/id.json", help="Path to operator keypair"
    )


def pytest_configure(config):
    if "RUST_LOG" in os.environ:
        pytest.CONTRACTS_PATH = pathlib.Path("/opt/solidity")
    else:
        pytest.CONTRACTS_PATH = pathlib.Path(__file__).parent / "contracts"


@pytest.fixture(scope="session")
def evm_loader(request) -> EvmLoader:
    wallet = OperatorAccount(pathlib.Path(request.config.getoption("--operator-key")).expanduser().as_posix())
    loader = EvmLoader(wallet)
    return loader


@pytest.fixture(scope="session")
def operator_keypair(request, evm_loader) -> Keypair:
    """
    Initialized solana keypair with balance. Get private key from cli or ~/.config/solana/id.json
    """
    with open(pathlib.Path(request.config.getoption("--operator-key")).expanduser(), "r") as key:
        account = Keypair(json.load(key)[:32])

    return account


@pytest.fixture(scope="session")
def treasury_pool(evm_loader) -> TreasuryPool:
    index = 2
    address = create_treasury_pool_address(index)
    index_buf = index.to_bytes(4, 'little')
    return TreasuryPool(index, address, index_buf)


@pytest.fixture(scope="function")
def user_account(evm_loader) -> Caller:
    return make_new_user(evm_loader)
