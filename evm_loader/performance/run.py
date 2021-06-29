from solana_utils import *

from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx, Trx
# from eth_utils import abi
from web3.auto import w3
from eth_keys import keys as eth_keys
from web3 import Web3
import argparse
from eth_utils import abi
from base58 import b58decode
import random

# CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "contracts/")
CONTRACTS_DIR = "contracts/"
# evm_loader_id = os.environ.get("EVM_LOADER")
evm_loader_id = "GWDnHTcNoryfBpLzRFvpicwfBHDJecRyw5Jq3Mo8P6RR"
chain_id = 111

sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
contracts_file = "contracts.json"
accounts_file = "accounts.json"



class PerformanceTest():
    @classmethod
    def setUpClass(cls):
        print("\ntest_performance.py setUpClass")

        wallet = RandomAccount()
        print("wallet.get_acc().public_key()", wallet.get_acc().public_key())
        tx = client.request_airdrop(wallet.get_acc().public_key(), 10000 * 10 ** 9, commitment=Confirmed)
        confirm_transaction(client, tx["result"])

        if getBalance(wallet.get_acc().public_key()) == 0:
            print("request_airdrop error")
            exit(0)

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


def check_address_event(result, factory_eth, erc20_eth):
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

def check_transfer_event(result, erc20_eth, acc_from, acc_to, sum, return_code):
    assert(result['meta']['err'] == None)
    assert(len(result['meta']['innerInstructions']) == 1)
    assert(len(result['meta']['innerInstructions'][0]['instructions']) == 2)
    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    assert(data[:1] == b'\x06')  #  OnReturn
    assert(data[1:2] == return_code)    # 11 - Machine encountered an explict stop,
                                        # 12 - Machine encountered an explict return
    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    assert(data[:1] == b'\x07')  # 7 means OnEvent
    assert(data[1:21] == bytes.fromhex(erc20_eth))
    assert(data[21:29] == bytes().fromhex('%016x' % 3)[::-1])  # topics len
    assert(data[29:61] == abi.event_signature_to_log_topic('Transfer(address,address,uint256)'))  # topics
    assert(data[61:93] == bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_from))  # from
    assert(data[93:125] == bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_to))  # to
    assert(data[125:157] == bytes().fromhex("%064x" % sum))  # value

def get_filehash(factory, factory_code, factory_eth, acc):
    trx = Transaction()
    trx.add(
        TransactionInstruction(
            program_id=evm_loader_id,
            data=bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('get_hash()'),
            keys=[
                AccountMeta(pubkey=factory, is_signer=False, is_writable=True),
                AccountMeta(pubkey=factory_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))
    result = send_transaction(client, trx, acc)['result']

    assert(result['meta']['err'] == None)
    assert(len(result['meta']['innerInstructions']) == 1)
    assert(len(result['meta']['innerInstructions'][0]['instructions']) == 2)
    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    assert(data[:1] == b'\x06')  #  OnReturn
    assert(data[1] == 0x11)  # 11 - Machine encountered an explict stop

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    assert(data[:1] == b'\x07')  # 7 means OnEvent
    assert(data[1:21] == factory_eth)
    assert(data[21:29] == bytes().fromhex('%016x' % 1)[::-1])  # topics len
    hash = data[61:93]
    return hash


def get_trx(contract_eth, caller, caller_eth, input, pr_key):
    tx = {'to': contract_eth, 'value': 1, 'gas': 1, 'gasPrice': 1,
        'nonce': getTransactionCount(client, caller), 'data': input, 'chainId': chain_id}
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, pr_key)
    print (from_addr.hex())
    print (caller_eth.hex())
    assert (from_addr == caller_eth)
    return (from_addr, sign, msg)

def sol_instr_keccak(keccak_instruction):
    return TransactionInstruction(
        program_id=keccakprog,
        data=keccak_instruction,
        keys=[AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False)]
    )


def sol_instr_05(evm_instruction, contract, contract_code, caller):
    return TransactionInstruction(program_id=evm_loader_id,
                               data=bytearray.fromhex("05") + evm_instruction,
                               keys=[
                                   AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=contract_code, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
                                   AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                                   AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                               ])

def deploy_contracts(args):
    instance = PerformanceTest()
    instance.setUpClass()

    res = instance.loader.deploy(CONTRACTS_DIR + "Factory.binary", instance.caller)
    (factory, factory_eth, factory_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    erc20_filehash = get_filehash(factory, factory_code, factory_eth, instance.acc)
    print("factory", factory)
    print ("factory_eth", factory_eth.hex())
    print("factory_code", factory_code)
    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('create_erc20(bytes32)')
    receipt_map = []

    for i in range(args.count):
        print (" -- count", i)
        trx_count = getTransactionCount(client, factory)

        salt = bytes().fromhex("%064x" % int(trx_count + i))
        trx_data = func_name + salt
        erc20_ether = bytes(Web3.keccak(b'\xff' + factory_eth + salt + erc20_filehash)[-20:])

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

        trx.add(
            TransactionInstruction(
                program_id=evm_loader_id,
                data=trx_data,
                keys=[
                    AccountMeta(pubkey=factory, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=factory_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=instance.acc.public_key(), is_signer=True, is_writable=False),
                    AccountMeta(pubkey=erc20_id, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                    AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                    AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))
        res = client.send_transaction(trx, instance.acc,
                                         opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))

        receipt_map.append((str(erc20_id), erc20_ether, str(erc20_code), res["result"]))

    contracts = []
    for (erc20_id, erc20_ether, erc20_code, receipt) in receipt_map:
        confirm_transaction(client, receipt)
        result = client.get_confirmed_transaction(receipt)
        check_address_event(result['result'], factory_eth, erc20_ether)
        contracts.append((erc20_id, erc20_ether.hex(), erc20_code))

    with open(contracts_file, mode='w') as f:
        f.write(json.dumps(contracts))


def create_accounts(args):
    instance = PerformanceTest()
    instance.setUpClass()

    receipt_map = []
    for i in range(args.count):
        pr_key = w3.eth.account.from_key(random.randbytes(32))
        acc_eth = bytes().fromhex(pr_key.address[2:])
        trx = Transaction()
        (transaction, acc_sol) = instance.loader.createEtherAccountTrx(acc_eth)
        trx.add(transaction)
        res = client.send_transaction(trx, instance.acc,
                                      opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))
        receipt_map.append((acc_eth, acc_sol, pr_key.privateKey.hex()[2:], res['result']))

    ether_accounts = []
    for (acc_eth, acc_sol, pr_key_hex,  receipt) in receipt_map:
        confirm_transaction(client, receipt)
        result = client.get_confirmed_transaction(receipt)
        print(acc_eth.hex(), acc_sol)
        ether_accounts.append((acc_eth.hex(), pr_key_hex, acc_sol))

    with open(accounts_file, mode='w') as f:
        f.write(json.dumps(ether_accounts))

def mint(accounts, contracts, acc):
    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('mint(address,uint256)')

    receipt_map = []
    sum = args.count*1000 * 10 ** 18
    for (erc20_sol, erc20_eth_hex, erc20_code) in contracts:
        for (acc_eth_hex, _, acc_sol) in accounts:
            trx_data = func_name + \
                       bytes().fromhex("%024x" % 0 + acc_eth_hex) + \
                       bytes().fromhex("%064x" % sum)

            trx = Transaction()
            trx.add(
                TransactionInstruction(
                    program_id=evm_loader_id,
                    data=trx_data,
                    keys=[
                        AccountMeta(pubkey=erc20_sol, is_signer=False, is_writable=True),
                        AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                        AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                        AccountMeta(pubkey=acc_sol, is_signer=False, is_writable=True),
                        AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                        AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                ]))
            res = client.send_transaction(trx, acc,
                                             opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))

            receipt_map.append((erc20_eth_hex, acc_eth_hex, res["result"]))

    for (erc20_eth_hex, acc_eth_hex, receipt) in receipt_map:
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)
        # print(res)
        check_transfer_event(res['result'], erc20_eth_hex, bytes(20).hex(), acc_eth_hex, sum, b'\x11')


def transfer(accounts, contracts, acc):
    func_name = abi.function_signature_to_4byte_selector('transfer(address,uint256)')
    receipt_map = []
    sum  = 1
    count = 0
    ia = iter(accounts)
    ic = iter(contracts)

    while count < args.count:
        try:
            (erc20_sol, erc20_eth_hex, erc20_code) = next(ic)
        except StopIteration as err:
            ic = iter(contracts)
            continue
        try:
            (acc_eth_hex1, pr_key_hex1, acc_sol1) = next(ia)
        except StopIteration as err:
            ia = iter(accounts)
            (acc_eth_hex1, pr_key_hex1, acc_sol1) = next(ia)

        (acc_eth_hex2, _, _) = accounts[random.randint(0, len(accounts)-1)]
        if acc_eth_hex1 == acc_eth_hex2:
            continue

        pr_key = w3.eth.account.from_key(bytes.fromhex(pr_key_hex1))

        count = count + 1
        trx_data = func_name + \
                   bytes().fromhex("%024x" % 0 + acc_eth_hex2) + \
                   bytes().fromhex("%064x" % sum)
        (from_addr, sign,  msg) = get_trx(
            bytes().fromhex(erc20_eth_hex),
            acc_sol1,
            bytes().fromhex(acc_eth_hex1),
            trx_data,
            bytes.fromhex(pr_key_hex1)
        )

        trx = Transaction()
        trx.add(sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
        trx.add(sol_instr_05((from_addr + sign + msg), erc20_sol, erc20_code, acc_sol1))

        res = client.send_transaction(trx, acc,
                                      opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))
        receipt_map.append((erc20_eth_hex, acc_eth_hex1, acc_eth_hex2, res["result"]))

    for (erc20_eth_hex, acc_eth_hex1, acc_eth_hex2, receipt) in receipt_map:
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)
        # print(res['result'])
        check_transfer_event(res['result'], erc20_eth_hex, acc_eth_hex1, acc_eth_hex2, sum, b'\x12')


def create_transactions(args):
    instance = PerformanceTest()
    instance.setUpClass()

    with open(contracts_file, mode='r') as f:
        contracts = json.loads(f.read())
    with open(accounts_file, mode='r') as f:
        accounts = json.loads(f.read())

    # erc20.mint()
    mint(accounts, contracts, instance.acc)
    # erc20.transfer()
    transfer(accounts, contracts, instance.acc)


# parse program args
parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('--count', metavar="count of the transaction",  type=int,  help='count transaction (>=1)')
parser.add_argument('--step', metavar="step of the test", type=str,  help='deploy, create_acc, create_trx, send_trx')

args = parser.parse_args()


if args.step == "deploy":
    deploy_contracts(args)
elif args.step == "create_acc":
    create_accounts(args)
elif args.step == "create_trx":
    create_transactions(args)


