from dataclasses import dataclass

from base58 import b58encode
from sha3 import keccak_256
from solana.keypair import Keypair
from solana.publickey import PublicKey

from ..solana_utils import EvmLoader, solana_client, get_transaction_count
from ..eth_tx_utils import pack, make_instruction_data_from_tx
from ..conftest import Caller
from .constants import ACCOUNT_SEED_VERSION


@dataclass
class Contract:
    eth_address: bytes
    solana_address: PublicKey
    nonce: int
    code_solana_address: PublicKey
    seed: str


def create_contract_address(user: Caller, evm_loader: EvmLoader) -> Contract:
    # Create contract address from (caller_address, nonce)
    user_nonce = get_transaction_count(solana_client, user.solana_account_address)
    contract_eth_address = keccak_256(pack([user.eth_address, user_nonce or None])).digest()[-20:]
    contract_solana_address, contract_nonce = evm_loader.ether2program(contract_eth_address)
    contract_code_address, _ = evm_loader.ether2seed(contract_eth_address)
    seed = b58encode(ACCOUNT_SEED_VERSION + contract_eth_address).decode('utf8')
    print(f"Contract addresses: "
          f"  eth {contract_eth_address.hex()}, "
          f"  solana {contract_solana_address}"
          f"  code {contract_code_address}"
          f"  nonce {contract_nonce}"
          f"  user nonce {user_nonce}"
          f"  seed {seed}")
    return Contract(contract_eth_address, PublicKey(contract_solana_address), contract_nonce,
                    PublicKey(contract_code_address), seed)


def make_eth_transaction(to_addr: str, data: bytes, signer: Keypair, from_solana_user: PublicKey, user_eth_address: bytes):
    nonce = get_transaction_count(solana_client, from_solana_user)
    tx = {'to': to_addr, 'value': 0, 'gas': 9999999999, 'gasPrice': 0,
          'nonce': nonce, 'data': data, 'chainId': 111}
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, signer.secret_key[:32])
    assert from_addr == user_eth_address, (from_addr, user_eth_address)
    return from_addr, sign, msg, nonce
