import base58
import unittest
from eth_tx_utils import make_keccak_instruction_data, Trx, make_instruction_data_from_tx
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


def deploy_erc20(loader, location_hex, location_bin, mintId, balance_erc20, caller):
        ctor_init = str("%064x" % 0xa0) + \
                    str("%064x" % 0xe0) + \
                    str("%064x" % 0x9) + \
                    base58.b58decode(balance_erc20).hex() + \
                    base58.b58decode(mintId).hex() + \
                    str("%064x" % 0x1) + \
                    str("77%062x" % 0x00) + \
                    str("%064x" % 0x1) + \
                    str("77%062x" % 0x00)

        with open(location_hex, mode='r') as hex:
            binary = bytearray.fromhex(hex.read() + ctor_init)
            with open(location_bin, mode='wb') as bin:
                bin.write(binary)
                res = loader.deploy(location_bin, caller)
                return res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId']



class ERC20test(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = WalletAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoader(cls.wallet, evm_loader_id)

        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()

        print('cls.caller_ether:', cls.caller_ether.hex())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        print('cls.caller:', cls.caller)

        balance = getBalance(cls.acc.public_key())
        print('balance:', balance)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            caller = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done")
            print("solana caller:", caller, 'cls.caller:', cls.caller)

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        cls.trx_count = getTransactionCount(client, cls.caller)

        erc20_id_ether = keccak_256(rlp.encode((cls.caller_ether, cls.trx_count))).digest()[-20:]
        (cls.erc20Id_precalculated, _) = cls.loader.ether2program(erc20_id_ether)
        print("cls.erc20Id_precalculated:", cls.erc20Id_precalculated)

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
            return res[15:59]

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
            return res[17:61]

    @staticmethod
    def tokenMint(mint_id, recipient, amount, owner=None):
        spl = SplToken(solana_url)
        if owner is None:
            res = spl.call("mint {} {} {}".format(mint_id, amount, recipient))
        else:
            res = spl.call("mint {} {} {} --owner {}".format(mint_id, amount, recipient, owner))
        print("minting {} tokens for {}".format(amount, recipient))

    @staticmethod
    def tokenBalance(acc):
        spl = SplToken(solana_url)
        res = spl.call("balance --address {}".format(acc))
        return int(res.rstrip())

    def erc20_deposit(self, payer, amount, erc20, erc20_code, balance_erc20, mint_id, receiver_erc20):
        func_name = abi.function_signature_to_4byte_selector('deposit(uint256,address,uint256,uint256)')
        input = func_name + bytes.fromhex(
            base58.b58decode(payer).hex() +
            str("%024x" % 0) + receiver_erc20.hex() +
            self.acc.public_key()._key.hex() +
            "%064x" % amount
        )
        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = make_keccak_instruction_data(1, len(msg))
        evm_instruction = from_addr + sign + msg

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog,
                                   data=keccak_input,
                                   keys=[
                                       AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False,
                                                   is_writable=False),
                                   ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=payer, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][2]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert 0 != ret, 'erc20_deposit: FAIL'
        return ret

    def erc20_withdraw(self, receiver, amount, erc20, erc20_code, balance_erc20, mint_id):
        func_name = abi.function_signature_to_4byte_selector('withdraw(uint256,uint256)')
        input = func_name + bytes.fromhex(
            base58.b58decode(receiver).hex() +
            "%064x" % amount
        )
        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = make_keccak_instruction_data(1, len(msg))
        evm_instruction = from_addr + sign + msg

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_input, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][2]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        assert ret != 0, 'erc20_withdraw: FAIL'
        return ret

    def erc20_balance(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('balanceOf(address)')
        input = bytes.fromhex("03") + func_name + bytes.fromhex(
            str("%024x" % 0) +
            self.caller_ether.hex()
        )
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=input,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance = int.from_bytes(value, "big")
        return balance

    def erc20_transfer(self, erc20, erc20_code, eth_to, amount):
        func_name = abi.function_signature_to_4byte_selector('transfer(address,uint256)')
        input = func_name + bytearray.fromhex(
            str("%024x" % 0) + eth_to +
            "%064x" % amount
        )

        caller_trx_cnt = getTransactionCount(client, self.caller)

        trx_raw = {'to': solana2ether(erc20), 'value': 1, 'gas': 1, 'gasPrice': 1, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 111}
        (from_addr, sign, msg) = make_instruction_data_from_tx(trx_raw, self.acc.secret_key())
        keccak_input = make_keccak_instruction_data(1, len(msg))
        evm_instruction = from_addr + sign + msg

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_input, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))
        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][1]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        ret = int.from_bytes(value, "big")
        print('erc20_transfer:', 'OK' if ret != 0 else 'FAIL')
        return ret

    def erc20_balance_ext(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('balance_ext()')
        input = bytearray.fromhex("03") + func_name
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=input, keys=
            [
                AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        balance_address = base58.b58encode(value)
        return balance_address

    def erc20_mint_id(self, erc20, erc20_code):
        func_name = abi.function_signature_to_4byte_selector('mint_id()')
        input = bytearray.fromhex("03") + func_name
        trx = Transaction().add(
            TransactionInstruction(program_id=self.loader.loader_id, data=input, keys=
            [
                AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))

        result = send_transaction(client, trx, self.acc)
        print(result)
        src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
        data = base58.b58decode(src_data)
        instruction = data[0]
        self.assertEqual(instruction, 6)  # 6 means OnReturn
        self.assertLess(data[1], 0xd0)  # less 0xd0 - success
        value = data[2:]
        mint_id = base58.b58encode(value)
        return mint_id

    def test_erc20(self):
        token = self.createToken()
        print("token:", token)

        balance_erc20 = self.createTokenAccount(token, self.erc20Id_precalculated)
        print("balance_erc20:", balance_erc20)

        (erc20Id, erc20Id_ether, erc20_code) = deploy_erc20(self.loader
                                                            , CONTRACTS_DIR+"ERC20.bin"
                                                            , "erc20.binary"
                                                            , token
                                                            , balance_erc20
                                                            , self.caller)
        print("erc20_id:", erc20Id)
        print("erc20_id_ethereum:", erc20Id_ether.hex())
        print("erc20_code:", erc20_code)

        assert (self.erc20Id_precalculated == erc20Id)
        assert(balance_erc20 == self.erc20_balance_ext(erc20Id, erc20_code).decode("utf-8"))
        assert(token == self.erc20_mint_id(erc20Id, erc20_code).decode("utf-8"))

        client_acc = self.createTokenAccount(token)

        mint_amount = 100
        self.tokenMint(token, client_acc, mint_amount)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (self.erc20_balance(erc20Id, erc20_code) == 0)

        deposit_amount = 1
        self.erc20_deposit(client_acc, deposit_amount * (10 ** 9), erc20Id
                           , erc20_code, balance_erc20, token, self.caller_ether)
        assert (self.tokenBalance(client_acc) == mint_amount - deposit_amount)
        assert (self.tokenBalance(balance_erc20) == deposit_amount)
        assert (self.erc20_balance(erc20Id, erc20_code) == deposit_amount * (10 ** 9))
        self.erc20_withdraw(client_acc, deposit_amount * (10 ** 9), erc20Id, erc20_code, balance_erc20, token)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        assert (self.erc20_balance(erc20Id, erc20_code) == 0)

    @unittest.skip("not for CI")
    def test_deposit(self):
        print("test_deposit")
        client_acc = "297MLscTY5SC4pwpPzTaFQBY4ndHdY1h5jC5FG18RMg2"
        erc20Id = "2a5PhGUpnTsCgVL8TjZ5S3LU76pmUfVC5UBHre4yqs5a"
        balance_erc20 = "8VAcZVoXCQoXb74DGMftRpraMYqHK86qKZALmBopo36i"
        token = "8y9XyppKvAWyu2Ud4HEAH6jaEAcCCvE53wcmr92t9RJJ"
        receiver_erc20 = bytes.fromhex("0000000000000000000000000000000000000011")
        self.erc20_deposit(client_acc, 900, erc20Id, balance_erc20, token, receiver_erc20)

    @unittest.skip("not for CI")
    def test_with_draw(self):
        print("test_withdraw")
        client_acc = "297MLscTY5SC4pwpPzTaFQBY4ndHdY1h5jC5FG18RMg2"
        erc20Id = "2a5PhGUpnTsCgVL8TjZ5S3LU76pmUfVC5UBHre4yqs5a"
        balance_erc20 = "8VAcZVoXCQoXb74DGMftRpraMYqHK86qKZALmBopo36i"
        token = "8y9XyppKvAWyu2Ud4HEAH6jaEAcCCvE53wcmr92t9RJJ"
        self.erc20_withdraw(client_acc, 10, erc20Id, balance_erc20, token)

    @unittest.skip("not for CI")
    def test_balance_ext(self):
        print("test_balance_ext")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_balance_ext(erc20Id))

    @unittest.skip("not for CI")
    def test_mint_id(self):
        print("test_mint_id")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_mint_id(erc20Id))

    @unittest.skip("not for CI")
    def test_balance(self):
        print("test_balance")
        erc20Id = "JDjTbq2CRdpfa12uYcDVHpQXQk5YHcfyrML73z824Uww"
        print(self.erc20_balance(erc20Id))

    @unittest.skip("not for CI")
    def test_tranfer(self):
        print("test_transfer")
        erc20_id = 'JZsZZrB7BBpxVR1SckTQrJ63rETuSJzN3HacPfr2gVt'
        erc20_code = 'EwkHSJ2x254LAbxkYru19VaMdh7tDP54Lt99wXbPMzyy'
        self.erc20_transfer(erc20_id, erc20_code, "0000000000000000000000000000000000000011", 0)


if __name__ == '__main__':
    unittest.main()
