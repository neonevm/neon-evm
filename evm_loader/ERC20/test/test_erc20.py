from enum import Enum
import base58
import unittest
from solana.rpc import types
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
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
        value = data[2:]
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
        value = data[2:]
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
        value = data[2:]
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
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert 0 != ret, 'erc20.deposit: FAIL'
        return ether_trx.trx_data

    def withdraw(self, ether_caller, receiver, amount, balance_erc20, mint_id):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('withdraw(uint256,uint256)')
            + bytes.fromhex(base58.b58decode(receiver).hex()
                            +"%064x" % amount),
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
        value = data[2:]
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
        value = data[2:]
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


class SplToken:
    def __init__(self, url):
        self.url = url

    def call(self, arguments):
        cmd = 'spl-token --url {} {}'.format(self.url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: spl-token error {}".format(err))
            raise


class EthereumTransaction:
    """Encapsulate the all data of an ethereum transaction that should be executed."""

    def __init__(self, ether_caller, contract_account, contract_code_account, trx_data, account_metas=None, steps=500):
        self.ether_caller = ether_caller
        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        self.trx_data = trx_data
        self.trx_account_metas = account_metas
        self.iterative_steps = steps
        self._solana_ether_caller = None  # is created in NeonEvmClient.__create_instruction_data_from_tx
        self._storage = None  # is created in NeonEvmClient.__send_neon_transaction
        print('trx_data:', self.trx_data.hex())
        if self.trx_account_metas is not None:
            print('trx_account_metas:', *self.trx_account_metas, sep='\n')


class ExecuteMode(Enum):
    SINGLE = 0
    ITERATIVE = 1


class NeonEvmClient:
    """Encapsulate the interaction logic with evm_loader to execute an ethereum transaction."""

    def __init__(self, solana_wallet, evm_loader):
        self.mode = ExecuteMode.SINGLE
        self.solana_wallet = solana_wallet
        self.evm_loader = evm_loader

    def set_execute_mode(self, new_mode):
        self.mode = ExecuteMode(new_mode)

    def send_ethereum_trx(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        if self.mode is ExecuteMode.SINGLE:
            return self.send_ethereum_trx_single(ethereum_transaction)
        if self.mode is ExecuteMode.ITERATIVE:
            return self.send_ethereum_trx_iterative(ethereum_transaction)

    def send_ethereum_trx_iterative(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        self.__send_neon_transaction(bytes.fromhex("09") +
                                     ethereum_transaction.iterative_steps.to_bytes(8, byteorder='little'),
                                     ethereum_transaction, need_storage=True)
        while True:
            result = self.__send_neon_transaction(bytes.fromhex("0A") +
                                                  ethereum_transaction.iterative_steps.to_bytes(8, byteorder='little'),
                                                  ethereum_transaction, need_storage=True)
            if result['result']['meta']['innerInstructions'] \
                    and result['result']['meta']['innerInstructions'][0]['instructions']:
                data = base58.b58decode(result['result']['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if data[0] == 6:
                    ethereum_transaction.__storage = None
                    return result

    def send_ethereum_trx_single(self, ethereum_transaction) -> types.RPCResponse:
        assert (isinstance(ethereum_transaction, EthereumTransaction))
        return self.__send_neon_transaction(bytes.fromhex("05"), ethereum_transaction)

    def __create_solana_ether_caller(self, ethereum_transaction):
        caller = self.evm_loader.ether2program(ethereum_transaction.ether_caller)[0]
        if ethereum_transaction._solana_ether_caller is None \
                or ethereum_transaction._solana_ether_caller != caller:
            ethereum_transaction._solana_ether_caller = caller
        if getBalance(ethereum_transaction._solana_ether_caller) == 0:
            print("Create solana ether caller account...")
            ethereum_transaction._solana_ether_caller = \
                self.evm_loader.createEtherAccount(ethereum_transaction.ether_caller)
        print("Solana ether caller account:", ethereum_transaction._solana_ether_caller)

    def __create_storage_account(self, seed):
        storage = PublicKey(
            sha256(bytes(self.solana_wallet.public_key())
                   + bytes(seed, 'utf8')
                   + bytes(PublicKey(self.evm_loader.loader_id))).digest())
        print("Storage", storage)

        if getBalance(storage) == 0:
            trx = Transaction()
            trx.add(createAccountWithSeed(self.solana_wallet.public_key(),
                                          self.solana_wallet.public_key(),
                                          seed, 10 ** 9, 128 * 1024,
                                          PublicKey(evm_loader_id)))
            send_transaction(client, trx, self.solana_wallet)
        return storage

    def __create_instruction_data_from_tx(self, ethereum_transaction):
        self.__create_solana_ether_caller(ethereum_transaction)
        caller_trx_cnt = getTransactionCount(client, ethereum_transaction._solana_ether_caller)
        trx_raw = {'to': solana2ether(ethereum_transaction.contract_account),
                   'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': ethereum_transaction.trx_data, 'chainId': 111}
        return make_instruction_data_from_tx(trx_raw, self.solana_wallet.secret_key())

    def __create_trx(self, ethereum_transaction, keccak_data, data):
        print('create_trx with keccak:', keccak_data.hex(), 'and data:', data.hex())
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=PublicKey(keccakprog), data=keccak_data, keys=
        [
            AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False),
        ]))
        trx.add(TransactionInstruction(program_id=self.evm_loader.loader_id, data=data, keys=
        [
            AccountMeta(pubkey=ethereum_transaction.contract_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=ethereum_transaction.contract_code_account, is_signer=False, is_writable=True),
            AccountMeta(pubkey=ethereum_transaction._solana_ether_caller, is_signer=False, is_writable=True),
            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.evm_loader.loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=self.solana_wallet.public_key(), is_signer=False, is_writable=False),
        ]))
        return trx

    def __send_neon_transaction(self, evm_trx_data, ethereum_transaction, need_storage=False) -> types.RPCResponse:
        (from_address, sign, msg) = self.__create_instruction_data_from_tx(ethereum_transaction)
        keccak_data = make_keccak_instruction_data(1, len(msg), 9 if need_storage else 1)
        data = evm_trx_data + from_address + sign + msg
        trx = self.__create_trx(ethereum_transaction, keccak_data, data)
        if need_storage:
            if ethereum_transaction._storage is None:
                ethereum_transaction._storage = self.__create_storage_account(sign[:8].hex())
            trx.instructions[-1].keys \
                .insert(0, AccountMeta(pubkey=ethereum_transaction._storage, is_signer=False, is_writable=True))
        if ethereum_transaction.trx_account_metas is not None:
            trx.instructions[-1].keys.extend(ethereum_transaction.trx_account_metas)
        trx.instructions[-1].keys \
            .append(AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False))
        return send_transaction(client, trx, self.solana_wallet)


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

        cls.trx_count = getTransactionCount(client, cls.caller)
        erc20_id_ether = keccak_256(rlp.encode((cls.ethereum_caller, cls.trx_count))).digest()[-20:]
        cls.erc20_id_precalculated = cls.loader.ether2program(erc20_id_ether)[0]
        print("erc20_id_precalculated:", cls.erc20_id_precalculated)

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

    # @unittest.skip("not for CI")
    def test_erc20(self):
        token = self.createToken()
        print("token:", token)

        balance_erc20 = self.createTokenAccount(token, self.erc20_id_precalculated)
        print("balance_erc20:", balance_erc20)

        erc20 = deploy_erc20(self.loader,
                             CONTRACTS_DIR + "ERC20.bin",
                             "erc20.binary",
                             token,
                             balance_erc20,
                             self.caller)

        assert (self.erc20_id_precalculated == erc20.contract_account)

        erc20.set_neon_evm_client(self.neon_evm_client)

        assert (balance_erc20 == erc20.balance_ext(self.ethereum_caller).decode("utf-8"))
        assert (token == erc20.mint_id(self.ethereum_caller).decode("utf-8"))

        client_acc = self.createTokenAccount(token)
        print("client_acc:", client_acc)

        mint_amount = 100
        self.tokenMint(token, client_acc, mint_amount)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (erc20.balance(self.ethereum_caller) == 0)

        deposit_amount = 1
        deposit_trx_data = erc20.deposit(self.ethereum_caller, client_acc, self.ethereum_caller,
                                         deposit_amount * (10 ** 9), balance_erc20, token,
                                         self.acc.public_key()._key)

        # check deposit emulation
        print('\nCheck deposit emulation:')
        print('ethereum_caller:', self.ethereum_caller.hex())
        print('erc20.ethereum_id:', erc20.ethereum_id.hex())
        print('deposit_trx_data:', deposit_trx_data.hex())
        args = self.ethereum_caller.hex() + ' ' + erc20.ethereum_id.hex() + ' ' + deposit_trx_data.hex()
        cli = neon_cli()
        cli_result = cli.emulate(evm_loader_id, args)
        print('cli_result:', cli_result)
        emulate_result = json.loads(cli_result)
        print('emulate_result:', emulate_result)
        assert (emulate_result["exit_status"] == 'succeed')
        assert(int(emulate_result['result']) == 1)
        print('deposit emulation: OK\n')

        assert (self.tokenBalance(client_acc) == mint_amount - deposit_amount)
        assert (self.tokenBalance(balance_erc20) == deposit_amount)
        assert (erc20.balance(self.ethereum_caller) == deposit_amount * (10 ** 9))
        erc20.withdraw(self.ethereum_caller, client_acc, deposit_amount * (10 ** 9), balance_erc20, token)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (erc20.balance(self.ethereum_caller) == 0)

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
