from typing import Union

from sha3 import keccak_256
from solana.keypair import Keypair
from solana.publickey import PublicKey
from web3.auto import w3

from .constants import CHAIN_ID

from .types import Caller, Contract
from ..eth_tx_utils import pack
from ..solana_utils import EvmLoader


def create_contract_address(user: Caller, evm_loader: EvmLoader) -> Contract:
    # Create contract address from (caller_address, nonce)
    user_nonce = evm_loader.get_neon_nonce(user.eth_address)
    contract_eth_address = keccak_256(pack([user.eth_address, user_nonce or None])).digest()[-20:]
    contract_solana_address, _ = evm_loader.ether2program(contract_eth_address)
    contract_neon_address = evm_loader.ether2balance(contract_eth_address)
    
    print(f"Contract addresses: "
          f"  eth {contract_eth_address.hex()}, "
          f"  solana {contract_solana_address}")

    return Contract(contract_eth_address, PublicKey(contract_solana_address), contract_neon_address)


def make_eth_transaction(to_addr: bytes, data: Union[bytes, None], caller: Caller,
                         value: int = 0, chain_id=CHAIN_ID, gas=9999999999, access_list=None, type=None):
    
    nonce = EvmLoader(caller.solana_account).get_neon_nonce(caller.eth_address)
    tx = {'to': to_addr, 'value': value, 'gas': gas, 'gasPrice': 0,
          'nonce': nonce}

    if chain_id is not None:
        tx['chainId'] = chain_id

    if data is not None:
        tx['data'] = data

    if access_list is not None:
        tx['accessList'] = access_list
    if type is not None:
        tx['type'] = type
    return w3.eth.account.sign_transaction(tx, caller.solana_account.secret_key[:32])
