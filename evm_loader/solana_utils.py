from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
import time
import os
import subprocess
from typing import NamedTuple
from construct import Bytes, Int8ul, Int32ul
from construct import Struct as cStruct
import json
from eth_keys import keys as eth_keys
import base64


solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
EVM_LOADER = os.environ.get("EVM_LOADER")
EVM_LOADER_SO = os.environ.get("EVM_LOADER_SO", 'target/bpfel-unknown-unknown/release/evm_loader.so')
http_client = Client(solana_url)
path_to_solana = 'solana'

def confirm_transaction(client, tx_sig):
    """Confirm a transaction."""
    TIMEOUT = 30  # 30 seconds  pylint: disable=invalid-name
    elapsed_time = 0
    while elapsed_time < TIMEOUT:
        resp = client.get_confirmed_transaction(tx_sig)
        if resp["result"]:
#            print('Confirmed transaction:', resp)
            break
        sleep_time = 3
        if not elapsed_time:
            sleep_time = 7
            time.sleep(sleep_time)
        else:
            time.sleep(sleep_time)
        elapsed_time += sleep_time
    if not resp["result"]:
        raise RuntimeError("could not confirm transaction: ", tx_sig)
    return resp



class SolanaCli:
    def __init__(self, url, acc):
        self.url = url
        self.acc = acc

    def call(self, arguments):
        cmd = '{} --keypair {} --url {} {}'.format(path_to_solana, self.acc.get_path(), self.url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise



class RandomAccount:
    def __init__(self, path=None):
        if path == None:
            self.make_random_path()
            print("New keypair file: {}".format(self.path))    
            self.generate_key()
        else:
            self.path = path
        self.retrieve_keys()
        print('New Public key:', self.acc.public_key())
        print('Private:', self.acc.secret_key())


    def make_random_path(self):
        import calendar;
        import time;
        ts = calendar.timegm(time.gmtime())
        self.path = str(ts) + '.json'
        time.sleep(1)
        

    def generate_key(self):
        cmd_generate = 'solana-keygen new --no-passphrase --outfile {}'.format(self.path)
        try:
            return subprocess.check_output(cmd_generate, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise

    def retrieve_keys(self):
        with open(self.path) as f:
            d = json.load(f)
            self.acc = Account(d[0:32])

    def get_path(self):
        return self.path

    def get_acc(self):
        return self.acc


class WalletAccount (RandomAccount):
    def __init__(self, path):
        self.path = path
        self.retrieve_keys()
        print('Wallet public key:', self.acc.public_key())



class EvmLoader:
    def __init__(self, solana_url, acc, programId=EVM_LOADER):
        if programId == None:
            print("Load EVM loader...")
            cli = SolanaCli(solana_url, acc)
            result = json.loads(cli.call('deploy {}'.format(EVM_LOADER_SO)))
            programId = result['programId']
        EvmLoader.loader_id = programId
        print("Done\n")

        self.solana_url = solana_url
        self.loader_id = EvmLoader.loader_id
        self.acc = acc
        print("Evm loader program: {}".format(self.loader_id))


    def deploy(self, contract):
        cli = SolanaCli(self.solana_url, self.acc)
        output = cli.call("deploy --use-evm-loader {} {}".format(self.loader_id, contract))
        print(type(output), output)
        result = json.loads(output.splitlines()[-1])
        return result


    def createEtherAccount(self, ether):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else: ether = ether.hex()
        cli = SolanaCli(self.solana_url, self.acc)
        output = cli.call("create-ether-account {} {} 1".format(self.loader_id, ether))
        result = json.loads(output.splitlines()[-1])
        return result["solana"]


    def ether2program(self, ether):
        if isinstance(ether, str):
            if ether.startswith('0x'): ether = ether[2:]
        else: ether = ether.hex()
        cli = SolanaCli(self.solana_url, self.acc)
        output = cli.call("create-program-address {} {}".format(ether, self.loader_id))
        items = output.rstrip().split('  ')
        return (items[0], int(items[1]))

    def checkAccount(self, solana):
        info = http_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))

    def deployChecked(self, location,  creator=None):
        from web3 import Web3
        if creator is None:
            creator = solana2ether("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r")
        with open(location, mode='rb') as file:
            fileHash = Web3.keccak(file.read())
            ether = bytes(Web3.keccak(b'\xff' + creator + bytes(32) + fileHash)[-20:])
        program = self.ether2program(ether)
        info = http_client.get_account_info(program[0])
        if info['result']['value'] is None:
            res = self.deploy(location)
            return (res['programId'], bytes.fromhex(res['ethereum'][2:]))
        elif info['result']['value']['owner'] != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return (program[0], ether)


def getBalance(account):
    return http_client.get_balance(account)['result']['value']

def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])


ACCOUNT_INFO_LAYOUT = cStruct(
    "eth_acc" / Bytes(20),
    "nonce" / Int8ul,
    "trx_count" / Bytes(8),
    "signer_acc" / Bytes(32),
    "code_size" / Int32ul
)

class AccountInfo(NamedTuple):
    eth_acc: eth_keys.PublicKey
    trx_count: int

    @staticmethod
    def frombytes(data):
        cont = ACCOUNT_INFO_LAYOUT.parse(data)
        return AccountInfo(cont.eth_acc, cont.trx_count)

def getAccountData(client, account, expected_length):
    info = client.get_account_info(account)['result']['value']
    if info is None:
        raise Exception("Can't get information about {}".format(account))

    data = base64.b64decode(info['data'][0])
    if len(data) != expected_length:
        raise Exception("Wrong data length for account data {}".format(account))
    return data


def getTransactionCount(client, sol_account):
    info = getAccountData(client, sol_account, ACCOUNT_INFO_LAYOUT.sizeof())
    acc_info = AccountInfo.frombytes(info)
    res = int.from_bytes(acc_info.trx_count, 'little')
    print('getTransactionCount {}: {}'.format(sol_account, res))
    return res

def wallet_path():
    cmd = 'solana --url {} config get'.format(solana_url)
    try:
        res =  subprocess.check_output(cmd, shell=True, universal_newlines=True)
        res = res.splitlines()[-1]
        substr = "Keypair Path: "
        if not res.startswith(substr):
            raise Exception("cannot get keypair path")
        path = res[len(substr):]
        return path.strip()
    except subprocess.CalledProcessError as err:
        import sys
        print("ERR: solana error {}".format(err))
        raise
