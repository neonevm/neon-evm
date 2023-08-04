import os
import json
import pathlib

import eth_abi
import pytest

from solana.keypair import Keypair
from eth_keys import keys as eth_keys
from solana.publickey import PublicKey
from solana.rpc.commitment import Confirmed

from .solana_utils import EvmLoader, OperatorAccount, create_treasury_pool_address, make_new_user, get_solana_balance, \
    deposit_neon, solana_client
from .utils.contract import deploy_contract
from .utils.storage import create_holder
from .utils.types import TreasuryPool, Caller, Contract


def pytest_addoption(parser):
    parser.addoption(
        "--operator-keys", action="store", default="~/.config/solana/id.json,~/.config/solana/id2.json",
        help="Path to 2 comma separated operator keypairs"
    )


def pytest_configure(config):
    if "RUST_LOG" in os.environ:
        pytest.CONTRACTS_PATH = pathlib.Path("/opt/solidity")
    else:
        pytest.CONTRACTS_PATH = pathlib.Path(__file__).parent / "contracts"


@pytest.fixture(scope="session")
def evm_loader(request) -> EvmLoader:
    wallet = OperatorAccount(
        pathlib.Path(request.config.getoption("--operator-keys").split(',')[0]).expanduser().as_posix())
    loader = EvmLoader(wallet)
    return loader


@pytest.fixture(scope="session")
def operator_keypair(request, evm_loader) -> Keypair:
    """
    Initialized solana keypair with balance. Get private key from cli or ~/.config/solana/id.json
    """
    with open(pathlib.Path(request.config.getoption("--operator-keys").split(',')[0]).expanduser(), "r") as key:
        secret_key = json.load(key)[:32]
        account = Keypair.from_secret_key(secret_key)
    caller_ether = eth_keys.PrivateKey(account.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)

    if get_solana_balance(PublicKey(caller)) == 0:
        evm_loader.check_account(account.public_key)
        evm_loader.check_account(PublicKey(caller))
        evm_loader.create_ether_account(caller_ether)
    return account


@pytest.fixture(scope="session")
def second_operator_keypair(request, evm_loader) -> Keypair:
    """
    Initialized solana keypair with balance. Get private key from cli or ~/.config/solana/id.json
    """
    with open(pathlib.Path(request.config.getoption("--operator-keys").split(",")[1]).expanduser(), "r") as key:
        secret_key = json.load(key)[:32]
        account = Keypair.from_secret_key(secret_key)
    caller_ether = eth_keys.PrivateKey(account.secret_key[:32]).public_key.to_canonical_address()
    caller, caller_nonce = evm_loader.ether2program(caller_ether)

    if get_solana_balance(PublicKey(caller)) == 0:
        evm_loader.create_ether_account(caller_ether)
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


@pytest.fixture(scope="session")
def session_user(evm_loader) -> Caller:
    return make_new_user(evm_loader)


@pytest.fixture(scope="session")
def second_session_user(evm_loader) -> Caller:
    return make_new_user(evm_loader)


@pytest.fixture(scope="session")
def sender_with_tokens(evm_loader, operator_keypair) -> Caller:
    user = make_new_user(evm_loader)
    deposit_neon(evm_loader, operator_keypair, user.eth_address, 100000)
    return user


@pytest.fixture(scope="session")
def holder_acc(operator_keypair) -> PublicKey:
    return create_holder(operator_keypair)


@pytest.fixture(scope="function")
def new_holder_acc(operator_keypair) -> PublicKey:
    return create_holder(operator_keypair)


@pytest.fixture(scope="function")
def rw_lock_contract(evm_loader: EvmLoader, operator_keypair: Keypair, session_user: Caller,
                     treasury_pool) -> Contract:
    return deploy_contract(operator_keypair, session_user, "rw_lock.binary", evm_loader, treasury_pool)


@pytest.fixture(scope="function")
def rw_lock_caller(evm_loader: EvmLoader, operator_keypair: Keypair,
                   session_user: Caller, treasury_pool: TreasuryPool, rw_lock_contract: Contract) -> Contract:
    constructor_args = eth_abi.encode(['address'], [rw_lock_contract.eth_address.hex()])
    return deploy_contract(operator_keypair, session_user, "rw_lock_caller.binary", evm_loader,
                           treasury_pool, encoded_args=constructor_args)


@pytest.fixture(scope="function")
def string_setter_contract(evm_loader: EvmLoader, operator_keypair: Keypair, session_user: Caller,
                           treasury_pool) -> Contract:
    return deploy_contract(operator_keypair, session_user, "string_setter.binary", evm_loader, treasury_pool)
