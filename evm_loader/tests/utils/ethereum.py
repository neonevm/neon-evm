from dataclasses import dataclass
from typing import Union

from base58 import b58encode
from sha3 import keccak_256
from solana.keypair import Keypair
from solana.publickey import PublicKey
from web3.auto import w3

from .constants import ACCOUNT_SEED_VERSION
from .types import Caller, Contract
from ..eth_tx_utils import pack
from ..solana_utils import EvmLoader, solana_client, get_transaction_count




def create_contract_address(user: Caller, evm_loader: EvmLoader) -> Contract:
    # Create contract address from (caller_address, nonce)
    user_nonce = get_transaction_count(solana_client, user.solana_account_address)
    contract_eth_address = keccak_256(pack([user.eth_address, user_nonce or None])).digest()[-20:]
    contract_solana_address, contract_nonce = evm_loader.ether2program(contract_eth_address)
    seed = b58encode(ACCOUNT_SEED_VERSION + contract_eth_address).decode('utf8')
    print(f"Contract addresses: "
          f"  eth {contract_eth_address.hex()}, "
          f"  solana {contract_solana_address}"
          f"  nonce {contract_nonce}"
          f"  user nonce {user_nonce}"
          f"  seed {seed}")
    return Contract(contract_eth_address, PublicKey(contract_solana_address), contract_nonce, seed)


def make_eth_transaction(to_addr: bytes, data: Union[bytes, None], signer: Keypair, from_solana_user: PublicKey,
                         value: int = 0, chain_id = 111, gas = 9999999999):
    nonce = get_transaction_count(solana_client, from_solana_user)
    tx = {'to': to_addr, 'value': value, 'gas': gas, 'gasPrice': 0,
          'nonce': nonce}

    if chain_id is not None:
        tx['chainId'] = chain_id

    if data is not None:
        tx['data'] = data

    return w3.eth.account.sign_transaction(tx, signer.secret_key[:32])

