Intro
======

In this directory are located tests for the evm which sends transactions direct to the evm.
Tests are using python and py.test library for run tests.


Installation
------------

```bash
pip install -r requirements.txt
py.test ./ -s -v
```

Moreover, we can use additional command line keys:

1. --operator-keys - path to 2 comma separated operator keys (by default ~/.config/solana/id.json,~/.config/solana/id2.json)

Also we can configure some variables from environment variables:

1. SOLANA_URL - by default http://localhost:8899
2. EVM_LOADER - set evm loader address
3. NEON_TOKEN_MINT - ethereum token mint address


How to write tests
==================

Structure
---------

Test has a several helper directories and files:

1. contracts - place for all Solidity contracts used in tests
2. utils - place for utilities which divided by logic parts
3. conftest.py - common fixtures for all tests


Common fixtures
---------------

For more information about fixtures see [pytest-fixture](https://docs.pytest.org/en/latest/fixture.html).
In several words, fixtures are functions which are called before and after each test and has a scope parameter which setup how often this function will be called.
For example:

```python
import pytest

@pytest.fixture(scope="function")
def fixture1():
    print("Call fixture1")
    return "fixture1"


def test_one(fixture1):
    pass


def test_two(fixture1):
    pass
```

In this case "fixture1" will be called before each test.

```python
import pytest

@pytest.fixture(scope="session")
def fixture1():
    print("Call fixture1")
    return "fixture1"


def test_one(fixture1):
    pass


def test_two(fixture1):
    pass
```

In this case "fixture1" will be called only once before all tests.

We have a several common fixtures:

1. evm_loader - fixture for evm loader object
2. operator_keypair - solana Keypair for operator key
3. treasury_pool - created treasury pool
4. user_account - created user account with ethereum account


Tips
====

Generate eth contract function call data
---------------------------------------

```python
from eth_utils import abi
func_name = abi.function_signature_to_4byte_selector('unchange_storage(uint8,uint8)')
data = (func_name + bytes.fromhex("%064x" % 0x01) + bytes.fromhex("%064x" % 0x01))
```

uint8 parameters must be 64 bytes long
