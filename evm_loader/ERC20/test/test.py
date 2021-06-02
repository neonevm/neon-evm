import base58
import unittest
from eth_tx_utils import make_keccak_instruction_data, Trx
from web3.auto import w3
from solana_utils import *
from re import search
import time

tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")

""" Compile *.sol from src in https://remix.ethereum.org/ and you will get ERC20.json
    This is a part of ERC20.json
    methodIdentifiers:
    {
        "allowance(address,address)": "dd62ed3e",
        "approve(address,uint256)": "095ea7b3",
        "balanceOf(address)": "70a08231",
        "balance_ext()": "40b6674d",
        "decimals()": "313ce567",
        "decreaseAllowance(address,uint256)": "a457c2d7",
        "deposit(uint256,address,uint256,uint256)": "6f0372af",
        "increaseAllowance(address,uint256)": "39509351",
        "mint_id()": "e132a122",
        "name()": "06fdde03",
        "symbol()": "95d89b41",
        "totalSupply()": "18160ddd",
        "transfer(address,uint256)": "a9059cbb",
        "transferFrom(address,address,uint256)": "23b872dd",
        "withdraw(uint256,uint256)": "441a3e70"
    }
    To call a contract method use 'numbers' as a transaction data
"""


def confirm_transaction(client, tx_sig):
    """Confirm a transaction."""
    TIMEOUT = 30  # 30 seconds  pylint: disable=invalid-name
    elapsed_time = 0
    while elapsed_time < TIMEOUT:
        sleep_time = 3
        if not elapsed_time:
            sleep_time = 7
            time.sleep(sleep_time)
        else:
            time.sleep(sleep_time)
        resp = client.get_confirmed_transaction(tx_sig)
        if resp["result"]:
            # print('Confirmed transaction:', resp)
            break
        elapsed_time += sleep_time
    if not resp["result"]:
        raise RuntimeError("could not confirm transaction: ", tx_sig)
    return resp


class SplToken:
    def __init__(self, url):
        self.url = url

    def call(self, arguments):
        cmd = 'spl-token --url {} {}'.format(self.url, arguments)
        try:
            print("cmd:", cmd)
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: spl-token error {}".format(err))
            raise


class EvmLoaderERC20(EvmLoader):
    def deploy_erc20(self, location_hex, location_bin, mintId, balance_erc20, caller, caller_ether):
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
                return self.deployChecked(location_bin, caller, caller_ether)


def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])


def getBalance(account):
    return client.get_balance(account)['result']['value']


class EvmLoaderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = RandomAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoaderERC20(cls.wallet, evm_loader_id)
        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.caller_ether = solana2ether(cls.acc.public_key())
        print('cls.caller_ether:', cls.caller_ether.hex())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)
        print('cls.caller:', cls.caller)

        while getBalance(cls.acc.public_key()) == 0:
            tx = client.request_airdrop(cls.acc.public_key(), 10 * 10 ** 9)
            confirm_transaction(client, tx['result'])
            time.sleep(1)
            balance = client.get_balance(cls.acc.public_key())['result']['value']
            print('balance:', balance)

        info = client.get_account_info(cls.caller)
        if info['result']['value'] is None:
            print("Create solana caller account...")
            caller = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done")
            print("solana caller:", caller, 'cls.caller:', cls.caller)

        cls.caller_nonce = getTransactionCount(client, cls.caller)

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        # precalculate erc20Id
        erc20_id_ether = keccak_256(rlp.encode((cls.caller_ether, cls.caller_nonce))).digest()[-20:]
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
        print('res:', res)
        if not res.startswith("Creating account "):
            raise Exception("create account error %s" % res)
        else:
            return res[17:61]

    @staticmethod
    def changeOwner(acc, owner):
        spl = SplToken(solana_url)
        res = spl.call("authorize {} owner {}".format(acc, owner))
        pos = res.find("New owner: ")
        if owner != res[pos + 11:pos + 55]:
            raise Exception("change owner error")

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
        input = "6f0372af" + \
                base58.b58decode(payer).hex() + \
                str("%024x" % 0) + receiver_erc20.hex() + \
                self.acc.public_key()._key.hex() + \
                "%064x" % amount

        info = getAccountData(client, self.caller, ACCOUNT_INFO_LAYOUT.sizeof())
        caller_trx_cnt = int.from_bytes(AccountInfo.frombytes(info).trx_count, 'little')

        trx_raw = {'to': solana2ether(erc20), 'value': 0, 'gas': 0, 'gasPrice': 0, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 1}
        trx_signed = w3.eth.account.sign_transaction(trx_raw, self.caller_eth_pr_key.key)
        trx_parsed = Trx.fromString(trx_signed.rawTransaction)
        trx_rlp = trx_parsed.get_msg(trx_raw['chainId'])
        eth_sig = eth_keys.Signature(vrs=[1 if trx_parsed.v % 2 == 0 else 0, trx_parsed.r, trx_parsed.s]).to_bytes()
        keccak_instruction = make_keccak_instruction_data(1, len(trx_rlp))
        evm_instruction = self.caller_ether + eth_sig + trx_rlp

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog,
                                   data=keccak_instruction,
                                   keys=[
                                       AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False),
                                   ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=payer, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        print('sleeping...')
        time.sleep(11)
        print('send_transaction:', input)
        result = client.send_transaction(trx, self.acc)
        print('result:', result)
        result = confirm_transaction(client, result["result"])
        messages = result["result"]["meta"]["logMessages"]
        res = messages[messages.index("Program log: Succeed") + 1]
        if not res.startswith("Program log: "):
            raise Exception("Invalid program logs: no result")
        else:
            if int(res[13:], 16) == 1:
                print("deposit OK")
            else:
                print("deposit Fail")

    def erc20_withdraw(self, receiver, amount, erc20, erc20_code, balance_erc20, mint_id):
        input = bytearray.fromhex(
            "441a3e70" +
            base58.b58decode(receiver).hex() +
            "%064x" % amount
        )
        info = getAccountData(client, self.caller, ACCOUNT_INFO_LAYOUT.sizeof())
        caller_trx_cnt = int.from_bytes(AccountInfo.frombytes(info).trx_count, 'little')

        trx_raw = {'to': solana2ether(erc20), 'value': 0, 'gas': 0, 'gasPrice': 0, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 1}
        trx_signed = w3.eth.account.sign_transaction(trx_raw, self.caller_eth_pr_key.key)
        trx_parsed = Trx.fromString(trx_signed.rawTransaction)
        trx_rlp = trx_parsed.get_msg(trx_raw['chainId'])
        eth_sig = eth_keys.Signature(vrs=[1 if trx_parsed.v % 2 == 0 else 0, trx_parsed.r, trx_parsed.s]).to_bytes()
        keccak_instruction = make_keccak_instruction_data(1, len(trx_rlp))
        evm_instruction = self.caller_ether + eth_sig + trx_rlp

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])).add(
            TransactionInstruction(program_id=self.loader.loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=balance_erc20, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=receiver, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=mint_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=tokenkeg, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=self.acc.public_key(), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = client.send_transaction(trx, self.acc)
        result = confirm_transaction(client, result["result"])
        messages = result["result"]["meta"]["logMessages"]
        res = messages[messages.index("Program log: Succeed") + 1]
        if not res.startswith("Program log: "):
            raise Exception("Invalid program logs: no result")
        else:
            if int(res[13:], 16) == 1:
                print("wirdraw OK")
            else:
                print("wirdraw Fail")

    def erc20_balance(self, erc20, erc20_code):
        input = bytearray.fromhex(
            "0370a08231" +
            str("%024x" % 0) + self.caller_ether.hex()
        )
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

        result = client.send_transaction(trx, self.acc)
        result = confirm_transaction(client, result["result"])
        print("result:", result)
        messages = result["result"]["meta"]["logMessages"]
        res = messages[-1]
        print("res:", res)
        if any(search("Program %s failed" % evm_loader_id, m) for m in messages):
            raise Exception("Invalid program logs: Program %s failed" % evm_loader_id)
        else:
            src_data = result['result']['meta']['innerInstructions'][0]['instructions'][0]['data']
            print('src_data:', src_data)
            data = base58.b58decode(src_data)
            print('data:', data, data.hex())
            instruction = data[0]
            print('instruction:', instruction)
            self.assertEqual(instruction, 6)  # 6 means OnReturn
            self.assertLess(data[1], 0xd0)  # less 0xd0 - success
            value = data[2:]
            print('value:', value, value.hex())
            balance = int.from_bytes(value, "little")
            print('balance:', balance)
            return balance

    def erc20_transfer(self, erc20, erc20_code, eth_to, amount):
        input = bytearray.fromhex(
            "a9059cbb" +
            str("%024x" % 0) + eth_to +
            "%064x" % amount
        )

        info = getAccountData(client, self.caller, ACCOUNT_INFO_LAYOUT.sizeof())
        caller_trx_cnt = int.from_bytes(AccountInfo.frombytes(info).trx_count, 'little')

        trx_raw = {'to': solana2ether(erc20), 'value': 0, 'gas': 0, 'gasPrice': 0, 'nonce': caller_trx_cnt,
                   'data': input, 'chainId': 1}
        trx_signed = w3.eth.account.sign_transaction(trx_raw, self.caller_eth_pr_key.key)
        trx_parsed = Trx.fromString(trx_signed.rawTransaction)
        trx_rlp = trx_parsed.get_msg(trx_raw['chainId'])
        eth_sig = eth_keys.Signature(vrs=[1 if trx_parsed.v % 2 == 0 else 0, trx_parsed.r, trx_parsed.s]).to_bytes()
        keccak_instruction = make_keccak_instruction_data(1, len(trx_rlp))
        evm_instruction = self.caller_ether + eth_sig + trx_rlp

        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
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
        result = client.send_transaction(trx, self.acc)
        result = confirm_transaction(client, result["result"])
        messages = result["result"]["meta"]["logMessages"]
        res = messages[-1]
        print("res:", res)
        if any(search("Program %s failed" % evm_loader_id, m) for m in messages):
            raise Exception("Invalid program logs: Program %s failed" % evm_loader_id)
        else:
            return res

    def erc20_balance_ext(self, erc20, erc20_code):
        input = bytearray.fromhex("0340b6674d")
        print("input:", list(input))
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
        print("trx:", trx)

        result = client.send_transaction(trx, self.acc)
        print("result:", result)
        confirm_result = confirm_transaction(client, result["result"])
        print("confirm_result:", confirm_result)
        log_messages = confirm_result["result"]["meta"]["logMessages"]
        print("log_messages:", log_messages)
        res = log_messages[-1]
        print("res:", res)
        if any(search("Program %s failed" % evm_loader_id, lm) for lm in log_messages):
            raise Exception("Invalid program logs: Program %s failed" % evm_loader_id)
        else:
            return res

    def erc20_mint_id(self, erc20, erc20_code):
        input = bytearray.fromhex("03e132a122")
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

        result = client.send_transaction(trx, self.acc)
        result = confirm_transaction(client, result["result"])
        messages = result["result"]["meta"]["logMessages"]
        res = messages[-1]
        print("res:", res)
        if any(search("Program %s failed" % evm_loader_id, m) for m in messages):
            raise Exception("Invalid program logs: Program %s failed" % evm_loader_id)
        else:
            return res

    def test_erc20(self):
        token = self.createToken()
        print("token:", token)

        balance_erc20 = self.createTokenAccount(token)
        print("balance_erc20:", balance_erc20)
        print('create account balance_erc20 = {balance_erc20} for erc20Id_precalculated = {erc20Id_precalculated}:'
              .format(balance_erc20=balance_erc20, erc20Id_precalculated=self.erc20Id_precalculated))

        (erc20Id, erc20Id_ether, erc20_code) = self.loader.deploy_erc20("erc20_ctor_uninit.hex"
                                                                        , "erc20.bin"
                                                                        , token
                                                                        , balance_erc20
                                                                        , self.caller
                                                                        , self.caller_ether)
        print("erc20_id:", erc20Id)
        print("erc20_id_ethereum:", erc20Id_ether.hex())
        print("erc20_code:", erc20_code)
        assert (self.erc20Id_precalculated == erc20Id)

        print("erc20 balance_ext():", self.erc20_balance_ext(erc20Id, erc20_code))
        print("erc20 mint_id():", self.erc20_mint_id(erc20Id, erc20_code))

        client_wallet = RandomAccount()
        client_public_key = client_wallet.get_acc().public_key()
        print("client public key:", client_public_key)
        client_acc = self.createTokenAccount(token, client_public_key)

        print('create account = {client_acc} for client public key = {client_public_key}:'
              .format(client_acc=client_acc, client_public_key=client_public_key))

        # self.changeOwner(balance_erc20, erc20Id)
        # print("balance_erc20 owner changed to {}".format(erc20Id))
        mint_amount = 100
        self.tokenMint(token, client_acc, mint_amount)
        assert (self.tokenBalance(client_acc) == mint_amount)
        assert (self.tokenBalance(balance_erc20) == 0)
        erc20_balance = self.erc20_balance(erc20Id, erc20_code)
        print('erc20_balance:', erc20_balance)
        assert (erc20_balance == 0)

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
        erc20Id = "9EWuA4YE7ABVKQEg1CChcQdozi93w5kLjo8wn3ZB9NKy"
        self.erc20_transfer(erc20Id, "0000000000000000000000000000000000000011", 0)


if __name__ == '__main__':
    unittest.main()
