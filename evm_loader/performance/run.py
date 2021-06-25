# from base58 import b58decode
import sys
from solana_utils import *

# from ..eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, Trx
# from eth_utils import abi
# from web3.auto import w3
from eth_keys import keys as eth_keys
# from web3 import Web3
# from multiprocessing import Process, Pool
# from enum import Enum
import random
import argparse
from eth_utils import abi
from base58 import b58decode


#
#
# solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
# http_client = Client(solana_url)
# # CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
# CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "contracts/")
CONTRACTS_DIR = "contracts/"
# # CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "")
# evm_loader_id = os.environ.get("EVM_LOADER")
evm_loader_id = "4XS7MwWXNjYjuTKo1KJN92MmSeM4Cw67eihEfcnzUZPP"
#
# sysinstruct = "Sysvar1nstructions1111111111111111111111111"
# keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"

class PerformanceTest():
    @classmethod
    def setUpClass(cls):
        print("\ntest_performance.py setUpClass")

        # wallet = WalletAccount(wallet_path())
        wallet = RandomAccount()
        tx = client.request_airdrop(wallet.acc.public_key(), 10000 * 10 ** 9)
        confirm_transaction(client, tx['result'])

        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))


    # def test_stages(self, stage: Stage = Stage.create_eth_acc, count = 2):
    #     if stage == Stage.deploy_erc20:
    #         addr = deploy_erc20(self.loader, self.caller, 2)
    #         for i in addr:
    #             print(i)
    #         print("deploy complete")
    #     elif stage == Stage.create_eth_acc:
    #         sol_addr = []
    #         eth_addr = []
    #         for i in range(count):
    #             eth_addr.append(random.randbytes(20))
    #             print(eth_addr[-1].hex())
    #             sol_addr.append(self.loader.createEtherAccount(eth_addr[-1]))
    #     # elif stage == Stage.create_eth_trx:
    #     #     for
#
# if __name__ == '__main__' and __package__ is None:
#     from os import sys, path
#     sys.path.append(path.dirname(path.dirname(path.abspath(__file__))))


def check_event(result, factory_eth, erc20_eth):
    assert(result['meta']['err'] == None)
    assert(len(result['meta']['innerInstructions']) == 2)
    assert(len(result['meta']['innerInstructions'][1]['instructions']) == 2)
    data = b58decode(result['meta']['innerInstructions'][1]['instructions'][1]['data'])
    assert(data[:1] == b'\x06')  #  OnReturn
    assert(data[1] == 0x11)  # 11 - Machine encountered an explict stop

    data = b58decode(result['meta']['innerInstructions'][1]['instructions'][0]['data'])
    assert(data[:1] == b'\x07')  # 7 means OnEvent
    assert(data[1:21] == factory_eth)
    assert(data[21:29] == bytes().fromhex('%016x' % 1)[::-1])  # topics len
    assert(data[29:61] == abi.event_signature_to_log_topic('Address(address)'))  # topics
    assert(data[61:93] == bytes().fromhex("%024x" % 0)+erc20_eth)  # sum


parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('--count', metavar="count of the transaction",  type=int,  help='count transaction (>=1)')
parser.add_argument('--step', metavar="step of the test", type=str,  help='deploy, create_acc, create_trx, send_trx')

args = parser.parse_args()
print(args.count)

if args.step == "deploy":
    print(args.step)
    instance = PerformanceTest()
    instance.setUpClass()

    print (CONTRACTS_DIR + "factory.binary")

    res = instance.loader.deploy(CONTRACTS_DIR + "Factory.binary", instance.caller)
    (factory, factory_eth, factory_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    print("factory", factory)
    print ("factory_eth", factory_eth.hex())
    print("factory_code", factory_code)
    call_create_erc20 = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('create_erc20()')


    for  i in range(args.count):
        print (" -- count", i)
        trx_count = getTransactionCount(client, factory)

        erc20_ether = keccak_256(rlp.encode((factory_eth, trx_count))).digest()[-20:]
        erc20_id = instance.loader.ether2program(erc20_ether)[0]
        seed = b58encode(bytes.fromhex(erc20_ether.hex()))
        erc20_code = accountWithSeed(instance.acc.public_key(), str(seed, 'utf8'), PublicKey(evm_loader_id))
        print("erc20_id:", erc20_id)
        print("erc20_eth:", erc20_ether.hex())
        print("erc20_code:", erc20_code)

        trx = Transaction()
        trx.add(
            createAccountWithSeed(
                instance.acc.public_key(),
                instance.acc.public_key(),
                str(seed, 'utf8'),
                10 ** 9,
                4000 + 4 * 1024,
                PublicKey(evm_loader_id))
        )
        trx.add(instance.loader.createEtherAccountTrx(erc20_ether, erc20_code)[0])

        # result = send_transaction(client, trx, instance.acc)
        # print (result)
        # trx = Transaction()
        trx.add(
            TransactionInstruction(
                program_id=evm_loader_id,
                data=call_create_erc20,
                keys=[
                    AccountMeta(pubkey=factory, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=factory_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=instance.acc.public_key(), is_signer=True, is_writable=False),
                    AccountMeta(pubkey=erc20_id, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))
        res = send_transaction(client, trx, instance.acc)
        check_event(res['result'], factory_eth, erc20_ether)

