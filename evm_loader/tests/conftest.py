import json
import pathlib

import pytest

from solana.keypair import Keypair

from solana_utils import EvmLoader, OperatorAccount


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
def evm_loader(request):
    wallet = OperatorAccount(pathlib.Path(request.config.getoption("--operator-key")).expanduser().as_posix())
    loader = EvmLoader(wallet)
    return loader
