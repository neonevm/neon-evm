from hashlib import sha256
from random import randrange

from sha3 import keccak_256
from solana.publickey import PublicKey
from solana.keypair import Keypair
from ..solana_utils import get_solana_balance, create_account_with_seed, \
    send_transaction, solana_client
from .instructions import TransactionWithComputeBudget
from .constants import EVM_LOADER


def create_storage_account(signer: Keypair, seed: bytes = None, size: int = None, fund: int = None) -> PublicKey:
    print(f"Create storage account with seed: {seed}")
    if size is None:
        size = 128 * 1024
    if fund is None:
        fund = 10 ** 9
    if seed is None:
        seed = str(randrange(1000000))
    storage = PublicKey(
        sha256(bytes(signer.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())

    if get_solana_balance(storage) == 0:
        trx = TransactionWithComputeBudget()
        trx.add(create_account_with_seed(signer.public_key, signer.public_key, seed, fund, size))
        send_transaction(solana_client, trx, signer)
    print(f"Created storage account: {storage}")
    return storage


def create_holder_account(operator: Keypair, holder_id: int = 0) -> (PublicKey, int):
    holder_id_bytes = holder_id.to_bytes((holder_id.bit_length() + 7) // 8, 'big')
    seed = keccak_256(b'holder' + holder_id_bytes).hexdigest()[:32]
    holder_account = create_storage_account(operator, seed, size=128*1024, fund=10**9)
    print(f"Created holder account: {holder_account}")
    return holder_account, holder_id
