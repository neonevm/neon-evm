import unittest
from eth_utils import abi
from web3.auto import w3
from solana_utils import *

tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/ERC20/src")


class ExternalCall:
    """Encapsulate the all data of the ExternalCall ethereum contract."""
    def __init__(self, contract_account, contract_code_account, ethereum_id=None):
        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        print("contract_id:", self.contract_account)
        print("contract_code:", self.contract_code_account)
        if ethereum_id is not None:
            self.ethereum_id = ethereum_id
            print("contract_ethereum:_id", self.ethereum_id.hex())
        self.neon_evm_client = None

    def set_neon_evm_client(self, neon_evm_client):
        self.neon_evm_client = neon_evm_client

    def transfer_ext(self, ether_caller, token, acc_from, acc_to, amount, signer):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('transferExt(uint256,uint256,uint256,uint256,uint256)')
            + bytes.fromhex(base58.b58decode(token).hex()
                            + base58.b58decode(acc_from).hex()
                            + base58.b58decode(acc_to).hex()
                            + "%064x" % amount
                            + signer.hex()
                            ),
            [
                AccountMeta(pubkey=acc_from, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc_to, is_signer=False, is_writable=True),
                AccountMeta(pubkey=token, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(tokenkeg), is_signer=False, is_writable=False),
            ])
        result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        return ether_trx.trx_data


def check_deposit_emulation(sender, contract, trx_data, accounts_should_be):
    print('\nCheck deposit emulation:')
    print('sender:', sender)
    print('contract:', contract)
    print('trx_data:', trx_data)
    cli = neon_cli()
    cli_result = cli.emulate(evm_loader_id, sender + ' ' + contract + ' ' + trx_data)
    print('cli_result:', cli_result)
    emulate_result = json.loads(cli_result)
    print('emulate_result:', emulate_result)
    assert (emulate_result["exit_status"] == 'succeed')
    assert (emulate_result['result'] == '')  # no type return from transferExt
    print('accounts_should_be:', accounts_should_be)
    accounts_not_found = len(accounts_should_be)
    for item in emulate_result["accounts"]:
        checking_account = [item["account"], item["address"], item["contract"]]
        exists = checking_account in accounts_should_be
        print('checking_account:', checking_account, exists)
        accounts_not_found -= exists
    print('accounts_not_found:', accounts_not_found)
    assert (accounts_not_found == 0)
    print('deposit emulation: OK\n')


class ERC20test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = WalletAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoader(cls.wallet, evm_loader_id)

        cls.neon_evm_client = NeonEvmClient(cls.acc, cls.loader)
        cls.neon_evm_client.set_execute_mode(ExecuteMode.ITERATIVE)

        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.ethereum_caller = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.ethereum_caller)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            cls.loader.createEtherAccount(cls.ethereum_caller)
            print("Done\n")

        print('Account: {} ({})'.format(cls.acc.public_key(), bytes(cls.acc.public_key()).hex()))
        print('Ethereum Caller: {}-{}'.format(cls.ethereum_caller.hex(), cls.caller_nonce))
        print('Solana Caller: {} ({})'.format(cls.caller, bytes(PublicKey(cls.caller)).hex()))

        res = cls.loader.deploy(CONTRACTS_DIR + "ExternalCall.binary", cls.caller)
        cls.contract = ExternalCall(res['programId'], res['codeId'], bytes.fromhex(res['ethereum'][2:]))
        cls.contract.set_neon_evm_client(cls.neon_evm_client)

    @staticmethod
    def createToken(owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("create-token")
        else:
            res = spl.call("create-token --owner {}".format(owner))
        if not res.startswith("Creating token "):
            raise Exception("create token error")
        else:
            return res.split()[2]

    @staticmethod
    def createTokenAccount(token, owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("create-account {}".format(token))
        else:
            res = spl.call("create-account {} --owner {}".format(token, owner))
        if not res.startswith("Creating account "):
            raise Exception("create account error %s" % res)
        else:
            return res.split()[2]

    @staticmethod
    def tokenMint(mint_id, recipient, amount, owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            spl.call("mint {} {} {}".format(mint_id, amount, recipient))
        else:
            spl.call("mint {} {} {} --owner {}".format(mint_id, amount, recipient, owner))
        print("minting {} tokens for {}".format(amount, recipient))

    @staticmethod
    def tokenBalance(acc):
        spl = SplToken(solana_url)
        res = spl.call("balance --address {}".format(acc))
        return int(res.rstrip())

    def create_storage_account(self, seed):
        storage = PublicKey(
            sha256(bytes(self.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.acc.public_key(), self.acc.public_key(), seed, 10 ** 9, 128 * 1024,
                                          PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.acc)
        return storage

    def test_cli_emulate(self):
        token = self.createToken()
        print("token:", token)

        wallet2 = RandomAccount()
        acc2 = wallet2.get_acc()

        token_acc1 = self.createTokenAccount(token, wallet2.get_path())
        # token_acc1 = self.createTokenAccount(token, self.contract.contract_account)
        print("token_acc1:", token_acc1)

        token_acc2 = self.createTokenAccount(token)
        print("token_acc2:", token_acc2)

        mint_amount = 100
        self.tokenMint(token, token_acc2, mint_amount)
        assert (self.tokenBalance(token_acc1) == 0)
        assert (self.tokenBalance(token_acc2) == mint_amount)

        transfer_amount = 17
        trx_data = self.contract.transfer_ext(self.ethereum_caller,
                                              token, token_acc2, token_acc1, transfer_amount * (10 ** 9),
                                              acc2.public_key()._key)
                                              # self.acc.public_key()._key)

        assert (self.tokenBalance(token_acc1) == transfer_amount)
        assert (self.tokenBalance(token_acc2) == mint_amount - transfer_amount)

        check_deposit_emulation(self.ethereum_caller.hex(),
                                self.contract.ethereum_id.hex(),
                                trx_data.hex(),
                                [
                                    [self.contract.contract_account,
                                     '0x' + self.contract.ethereum_id.hex(),
                                     self.contract.contract_code_account],
                                    [self.caller,
                                     '0x' + self.ethereum_caller.hex(),
                                     None]
                                ])


if __name__ == '__main__':
    unittest.main()
