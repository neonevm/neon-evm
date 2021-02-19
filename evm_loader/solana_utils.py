from solana.rpc.api import Client
from solana.account import Account
from solana.publickey import PublicKey
import time
import os
import json
import subprocess

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
# path_to_patched_solana = 'solana'
# path_to_patched_solana = '../solana/target/debug/solana'
# path_to_patched_solana = 'solana'
path_to_patched_solana = '/home/dmitriy/cyber-core/solana/target/debug/solana'

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
#            print('Confirmed transaction:', resp)
            break
        elapsed_time += sleep_time
        if not resp["result"]:
            raise RuntimeError("could not confirm transaction: ", tx_sig)
    return resp



class SolanaCli:
    def __init__(self, url, acc):
        self.url = url
        self.acc = acc

    def call(self, arguments):
        cmd = '{} --keypair {} --url {} {}'.format(path_to_patched_solana, self.acc.get_path(), self.url, arguments)
        try:
            return subprocess.check_output(cmd, shell=True, universal_newlines=True)
        except subprocess.CalledProcessError as err:
            import sys
            print("ERR: solana error {}".format(err))
            raise



class RandomAccaunt:
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



class EvmLoader:
    def __init__(self, solana_url, acc, programId=None):
        if programId == None:
            print("Load EVM loader...")
            cli = SolanaCli(solana_url, acc)
            contract = 'target/bpfel-unknown-unknown/release/evm_loader.so'
            result = json.loads(cli.call('deploy {}'.format(contract)))
            programId = result['programId']
        EvmLoader.loader_id = programId
        # EvmLoader.loader_id = "3qEJUEkcbP5PEWzRxRYpa6yfVBm5yZmHkz7TzjRzUPhP"
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
        return result['programId']


    def createEtherAccount(self, ether):
        cli = SolanaCli(self.solana_url, self.acc)
        output = cli.call("create-ether-account {} {} 1".format(self.loader_id, ether.hex()))
        result = json.loads(output.splitlines()[-1])
        return result["solana"]


    def ether2program(self, ether):
        cli = SolanaCli(self.solana_url, self.acc)
        output = cli.call("create-program-address {} {}".format(ether.hex(), self.loader_id))
        items = output.rstrip().split('  ')
        return (items[0], int(items[1]))

    def checkAccount(self, solana):
        info = http_client.get_account_info(solana)
        print("checkAccount({}): {}".format(solana, info))

    def deployChecked(self, location):
        from web3 import Web3
        creator = solana2ether("6ghLBF2LZAooDnmUMVm8tdNK6jhcAQhtbQiC7TgVnQ2r")
        with open(location, mode='rb') as file:
            fileHash = Web3.keccak(file.read())
            ether = bytes(Web3.keccak(b'\xff' + creator + bytes(32) + fileHash)[-20:])
        program = self.ether2program(ether)
        info = http_client.get_account_info(program[0])
        if info['result']['value'] is None:
            return self.deploy(location)
        elif info['result']['value']['owner'] != self.loader_id:
            raise Exception("Invalid owner for account {}".format(program))
        else:
            return {"ethereum": ether.hex(), "programId": program[0]}


def getBalance(account):
    return http_client.get_balance(account)['result']['value']

def solana2ether(public_key):
    from web3 import Web3
    return bytes(Web3.keccak(bytes(PublicKey(public_key)))[-20:])
