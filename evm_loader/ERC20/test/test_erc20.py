from enum import Enum
import base58
import unittest
from solana.rpc import types
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from spl.token.constants import TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address
from eth_utils import abi
from web3.auto import w3
from solana_utils import *

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/ERC20/src")


class ERC20:
    """Encapsulate the all data of the ERC20 ethereum contract."""

    def __init__(self, contract_account, contract_code_account, ethereum_id=None):
        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        print("erc20_id:", self.contract_account)
        print("erc20_code:", self.contract_code_account)
        if ethereum_id is not None:
            self.ethereum_id = ethereum_id
            print("erc20_id_ethereum:", self.ethereum_id.hex())
        self.neon_evm_client = None

    def set_neon_evm_client(self, neon_evm_client):
        self.neon_evm_client = neon_evm_client

    def balance_ext(self, ether_caller):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('balance_ext()'))
        result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        balance_address = base58.b58encode(value)
        return balance_address

    def mint_id(self, ether_caller):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('mint_id()'))
        result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        mint_id = base58.b58encode(value)
        return mint_id

    def balance(self, ether_caller):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('balanceOf(address)')
            + bytes.fromhex(str("%024x" % 0) + ether_caller.hex()))
        result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        balance = int.from_bytes(value, "big")
        return balance

    def deposit(self, ether_caller, payer, receiver_erc20, amount, balance_erc20, mint_id, signer):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('deposit(uint256,address,uint256,uint256)')
            + bytes.fromhex(base58.b58decode(payer).hex()
                            + str("%024x" % 0) + receiver_erc20.hex()
                            + signer.hex()
                            + "%064x" % amount),
            [
                AccountMeta(pubkey=payer, is_signer=False, is_writable=True),
                AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(tokenkeg), is_signer=False, is_writable=False),
            ])
        result = self.neon_evm_client.send_ethereum_trx(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        ret = int.from_bytes(value, "big")
        assert 0 != ret, 'erc20.deposit: FAIL'
        return ether_trx.trx_data

    def withdraw(self, ether_caller, receiver, amount, balance_erc20, mint_id):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('withdraw(uint256,uint256)')
            + bytes.fromhex(base58.b58decode(receiver).hex()
                            + "%064x" % amount),
            [
                AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=receiver, is_signer=False, is_writable=True),
                AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(tokenkeg), is_signer=False, is_writable=False),
            ])
        result = self.neon_evm_client.send_ethereum_trx(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        ret = int.from_bytes(value, "big")
        assert ret != 0, 'erc20.withdraw: FAIL'
        return ret

    def transfer(self, ether_caller, recipient, amount):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('transfer(address,uint256)')
            + bytearray.fromhex(str("%024x" % 0)
                                + recipient
                                + "%064x" % amount))
        result = self.neon_evm_client.send_ethereum_trx(ether_trx)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        assert (instruction == 6)  # 6 means OnReturn
        assert (data[1] < 0xd0)  # less 0xd0 - success
        value = data[10:]
        ret = int.from_bytes(value, "big")
        print('erc20.transfer:', 'OK' if ret != 0 else 'FAIL')
        return ret


def deploy_erc20(loader, location_hex, location_bin, mint_id, balance_erc20, caller) -> ERC20:
    ctor_init = str("%064x" % 0xa0) + \
                str("%064x" % 0xe0) + \
                str("%064x" % 0x9) + \
                base58.b58decode(balance_erc20).hex() + \
                base58.b58decode(mint_id).hex() + \
                str("%064x" % 0x1) + \
                str("77%062x" % 0x00) + \
                str("%064x" % 0x1) + \
                str("77%062x" % 0x00)
    with open(location_hex, mode='r') as hex:
        binary = bytearray.fromhex(hex.read() + ctor_init)
        with open(location_bin, mode='wb') as bin:
            bin.write(binary)
            res = loader.deploy(location_bin, caller)
            return ERC20(res['programId'], res['codeId'], bytes.fromhex(res['ethereum'][2:]))


class ERC20test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = WalletAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoader(cls.wallet, evm_loader_id)
        cls.spl_token = SplToken(solana_url)
        cls.neon_evm_client = NeonEvmClient(cls.acc, cls.loader)
        cls.neon_evm_client.set_execute_mode(ExecuteMode.ITERATIVE)

        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.ethereum_caller = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.ethereum_caller)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            cls.loader.createEtherAccount(cls.ethereum_caller)
            cls.spl_token.transfer(ETH_TOKEN_MINT_ID, 2000, get_associated_token_address(PublicKey(cls.ethereum_caller), ETH_TOKEN_MINT_ID))
            print("Done\n")

        print('Account: {} ({})'.format(cls.acc.public_key(), bytes(cls.acc.public_key()).hex()))
        print('Ethereum Caller: {}-{}'.format(cls.ethereum_caller.hex(), cls.caller_nonce))
        print('Solana Caller: {} ({})'.format(cls.caller, bytes(PublicKey(cls.caller)).hex()))

        cls.trx_count = getTransactionCount(client, cls.caller)
        erc20_id_ether = keccak_256(rlp.encode((cls.ethereum_caller, cls.trx_count))).digest()[-20:]
        cls.erc20_id_precalculated = cls.loader.ether2program(erc20_id_ether)[0]
        print("erc20_id_precalculated:", cls.erc20_id_precalculated)

    # @unittest.skip("not for CI")
    def test_erc20(self):
        token = self.spl_token.create_token()
        print("token:", token)

        balance_erc20 = self.spl_token.create_token_account(token, self.erc20_id_precalculated)
        print("balance_erc20:", balance_erc20)

        erc20 = deploy_erc20(self.loader,
                             CONTRACTS_DIR + "ERC20.bin",
                             "erc20.binary",
                             token,
                             balance_erc20,
                             self.caller)

        self.assertEqual(self.erc20_id_precalculated, erc20.contract_account)

        erc20.set_neon_evm_client(self.neon_evm_client)

        self.assertEqual(balance_erc20, erc20.balance_ext(self.ethereum_caller).decode("utf-8"))
        self.assertEqual(token, erc20.mint_id(self.ethereum_caller).decode("utf-8"))

        client_acc = self.spl_token.create_token_account(token)
        print("client_acc:", client_acc)

        mint_amount = 100
        self.spl_token.mint(token, client_acc, mint_amount)
        self.assertEqual(self.spl_token.balance(client_acc), mint_amount)
        self.assertEqual(self.spl_token.balance(balance_erc20), 0)
        self.assertEqual(erc20.balance(self.ethereum_caller), 0)

        deposit_amount = 1
        erc20.deposit(self.ethereum_caller, client_acc, self.ethereum_caller,
                      deposit_amount * (10 ** 9), balance_erc20, token,
                      self.acc.public_key()._key)

        self.assertEqual(self.spl_token.balance(client_acc), mint_amount - deposit_amount)
        self.assertEqual(self.spl_token.balance(balance_erc20), deposit_amount)
        self.assertEqual(erc20.balance(self.ethereum_caller), deposit_amount * (10 ** 9))

        erc20.withdraw(self.ethereum_caller, client_acc, deposit_amount * (10 ** 9), balance_erc20, token)
        self.assertEqual(self.spl_token.balance(client_acc), mint_amount)
        self.assertEqual(self.spl_token.balance(balance_erc20), 0)
        self.assertEqual(erc20.balance(self.ethereum_caller), 0)

    @unittest.skip("not for CI")
    def test_deposit(self):
        print("test_deposit")
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        client_acc = "FzFxJHDaNG2tUUgmgTBjSuZEAA8JpCjmmPmuYN6xRfS2"
        balance_erc20 = "FwNEpebVFsQ1j54zbrFhWgWVXVa9Xdf5bV94PJ7Du5pN"
        token = "25AeYuTg2Uey4bYD6D5xjgEmoUXvbjQxZgEKF81p3NUN"
        receiver_erc20 = bytes.fromhex("0000000000000000000000000000000000000011")
        signer = self.acc.public_key()._key
        erc20.deposit(self.ethereum_caller, client_acc, receiver_erc20, 900, balance_erc20, token, signer)

    @unittest.skip("not for CI")
    def test_with_draw(self):
        print("test_withdraw")
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        client_acc = "297MLscTY5SC4pwpPzTaFQBY4ndHdY1h5jC5FG18RMg2"
        balance_erc20 = "8VAcZVoXCQoXb74DGMftRpraMYqHK86qKZALmBopo36i"
        token = "8y9XyppKvAWyu2Ud4HEAH6jaEAcCCvE53wcmr92t9RJJ"
        erc20.withdraw(self.ethereum_caller, client_acc, 10, balance_erc20, token)

    @unittest.skip("not for CI")
    def test_balance_ext(self):
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        print(erc20.balance_ext(self.ethereum_caller))

    @unittest.skip("not for CI")
    def test_mint_id(self):
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        print(erc20.mint_id(self.ethereum_caller))

    @unittest.skip("not for CI")
    def test_balance(self):
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        print(erc20.balance(self.ethereum_caller))

    @unittest.skip("not for CI")
    def test_tranfer(self):
        print("test_transfer")
        erc20_id = '5XVDY4xspYNLDvmshYtkGSi9QgqCb6aZ6w7W7cUMpzsy'
        erc20_code = 'ARUHVJE4zYws6cj8ThFNdVCQS1hafQi4zZiLc87NoDfF'
        erc20 = ERC20(erc20_id, erc20_code)
        erc20.set_neon_evm_client(self.neon_evm_client)
        erc20.transfer(self.ethereum_caller, "0000000000000000000000000000000000000011", 0)


if __name__ == '__main__':
    unittest.main()
