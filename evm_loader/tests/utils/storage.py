from hashlib import sha256
from random import randrange

from solana.publickey import PublicKey
from solana.keypair import Keypair
from ..solana_utils import create_holder_account, get_solana_balance, create_account_with_seed, \
    send_transaction, solana_client
from solana.transaction import Transaction
from .constants import EVM_LOADER


def create_holder(signer: Keypair, seed: str = None, size: int = None, fund: int = None,
                  storage: PublicKey = None) -> PublicKey:
    if size is None:
        size = 128 * 1024
    if fund is None:
        fund = 10 ** 9
    if seed is None:
        seed = str(randrange(1000000))
    if storage is None:
        storage = PublicKey(
            sha256(bytes(signer.public_key) + bytes(seed, 'utf8') + bytes(PublicKey(EVM_LOADER))).digest())

    print(f"Create holder account with seed: {seed}")

    if get_solana_balance(storage) == 0:
        trx = Transaction()
        trx.add(
            create_account_with_seed(signer.public_key, signer.public_key, seed, fund, size),
            create_holder_account(storage, signer.public_key)
        )
        send_transaction(solana_client, trx, signer)
    print(f"Created holder account: {storage}")
    return storage
