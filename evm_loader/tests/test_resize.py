import os
import web3

from solana.keypair import Keypair

from solana_utils import PublicKey, ACCOUNT_INFO_LAYOUT, EVM_LOADER, EvmLoader, Account, Transaction, get_account_data, \
    solana_client, AccountInfo, b58encode, ACCOUNT_SEED_VERSION, account_with_seed, create_account_with_seed, TransactionInstruction, \
    AccountMeta, TransactionWithComputeBudget, OperatorAccount, operator1_keypair_path, send_transaction


def create_resize_transaction(loader: EvmLoader, acc: Keypair, address: str, size: int) -> Transaction:
    solana_address = PublicKey(loader.ether2program(address)[0])
    account_data: bytes = get_account_data(solana_client, solana_address, ACCOUNT_INFO_LAYOUT.sizeof())
    account: AccountInfo = AccountInfo.from_bytes(account_data)

    seed = b58encode(ACCOUNT_SEED_VERSION + os.urandom(20)).decode('utf8')
    code_account_new = account_with_seed(acc.public_key, seed, PublicKey(EVM_LOADER))
    minimum_balance = solana_client.get_minimum_balance_for_rent_exemption(size)["result"]

    create_with_seed = create_account_with_seed(acc.public_key, acc.public_key, seed, minimum_balance, size,
                                                PublicKey(EVM_LOADER))
    resize = TransactionInstruction(
        program_id=EVM_LOADER,
        data=bytearray.fromhex("11") + seed.encode('utf-8'),  # 17- ResizeStorageAccount
        keys=[
            AccountMeta(pubkey=solana_address, is_signer=False, is_writable=True),
            AccountMeta(pubkey=account.code_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=code_account_new, is_signer=False, is_writable=True),
            AccountMeta(pubkey=acc.public_key, is_signer=True, is_writable=False)
        ]
    )

    trx = TransactionWithComputeBudget()
    trx.add(create_with_seed)
    trx.add(resize)

    return trx


class TestResize:
    loader: EvmLoader
    account: Keypair

    @classmethod
    def setup_class(cls):
        wallet = OperatorAccount(operator1_keypair_path())
        cls.loader = EvmLoader(wallet, EVM_LOADER)
        cls.account = Keypair.from_secret_key(wallet.get_acc().secret_key())

    def test_resize(self):
        account = web3.Account.create()
        self.loader.create_ether_account(account.address)

        for size in range(1*1024*1024, 10*1024*1024 + 1, 1*1024*1024):
            resize_trx = create_resize_transaction(self.loader, self.account, account.address, size)
            result = send_transaction(solana_client, resize_trx, self.account)
            assert result["status"] == {"OK": None}
