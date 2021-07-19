from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from web3.auto import w3
from eth_keys import keys as eth_keys
from web3 import Web3
import argparse
from eth_utils import abi
from base58 import b58decode
import random
from solana.blockhash import *
import statistics

factory_path = "Factory.binary"
evm_loader_id = os.environ.get("EVM_LOADER")
# evm_loader_id = "wkiSZ5TANo7e4MjaJhCYND9A7FQXHkoZNRcUjeuK5Yp"
chain_id = 111
transfer_sum = 1

sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
contracts_file = "contract.json"
accounts_file = "account.json"
transactions_file = "transaction.json"
senders_file = "sender.json"
verify_file = "verify.json"

# map caller->trx_count
trx_count = {}


class init_senders():
    @classmethod
    def init(cls):
        cls.accounts = []
        file = open(senders_file + args.postfix, mode='r')
        for line in file:
            # pair = bytes.fromhex(line)
            # rec = json.loads(line)
            # print (rec)
            cls.accounts.append(Account(bytes().fromhex(line[:64])))
        print("init_senders init")

        if len(cls.accounts) == 0:
            raise RuntimeError("solana senders is absent")
        cls.current = 0

    def next_acc(self):
        self.current = self.current + 1
        if self.current >= len(self.accounts):
            self.current = 0
        return self.accounts[self.current]


class init_wallet():
    @classmethod
    def init(cls):
        print("\ntest_performance.py init")

        wallet = RandomAccount()
        if getBalance(wallet.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        assert (getBalance(wallet.get_acc().public_key()) > 0)

        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()
        cls.keypath = wallet.get_path()

        # Create ethereum account for user account
        # cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.caller_ether = bytes.fromhex(cls.caller_eth_pr_key.address[2:])
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
    # assert(result['meta']['err'] == None)

    if (len(result['meta']['innerInstructions']) != 1):
        print("len(result['meta']['innerInstructions']) != 1", len(result['meta']['innerInstructions']))
        return False

    if (len(result['meta']['innerInstructions'][0]['instructions']) != 2):
        print("len(result['meta']['innerInstructions'][0]['instructions']) != 2",
              len(result['meta']['innerInstructions'][0]['instructions']))
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    if (data[:1] != b'\x06'):  #  OnReturn
        print("data[:1] != x06", data[:1].hex())
        return False

    if(data[1:2] != return_code):    # 11 - Machine encountered an explict stop,  # 12 - Machine encountered an explict return
        print("data[1:2] != return_code", data[1:2].hex(), return_code.hex())
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    if(data[:1] != b'\x07'):  # 7 means OnEvent
        print("data[:1] != x07", data[:1].hex())
        return  False


    if (data[1:21] != bytes.fromhex(erc20_eth)):
        print("data[1:21] != bytes.fromhex(erc20_eth)", data[1:21].hex(), erc20_eth)
        return False

    if(data[21:29] != bytes().fromhex('%016x' % 3)[::-1]):  # topics len
        print("data[21:29] != bytes().fromhex('%016x' % 3)[::-1]", data[21:29].hex())
        return False

    if(data[29:61] != abi.event_signature_to_log_topic('Transfer(address,address,uint256)')):  # topics
        print("data[29:61] != abi.event_signature_to_log_topic('Transfer(address,address,uint256)')",
              data[29:61].hex(),
              abi.event_signature_to_log_topic('Transfer(address,address,uint256)').hex())
        return False

    if (data[61:93] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_from)):
        print("data[61:93] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)",
              data[61:93].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)).hex())
        return False

    if(data[93:125] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_to)):  # from
        print("data[93:125] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)",
              data[93:125].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)).hex()
              )
        return False

    if (data[125:157] != bytes().fromhex("%064x" % sum)):  # value
        print("data[125:157] != bytes().fromhex('%064x' % sum)",
              data[125:157].hex(),
              '%064x' % sum)
        return False

    return True

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
    print(result)
    if result['meta']['err'] != None:
        print(result)
        print("Error: result['meta']['err'] != None")
        exit(1)

    if result == None:
        print("Error: result == None")
        exit(1)
        
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
    if trx_count.get(caller) != None:
        trx_count[caller] = trx_count[caller] + 1
    else:
        trx_count[caller] = getTransactionCount(client, caller)

    tx = {'to': contract_eth, 'value': 1, 'gas': 9999999999, 'gasPrice': 1,
        'nonce': trx_count[caller], 'data': input, 'chainId': chain_id}
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, pr_key)

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
    instance = init_wallet()
    instance.init()

    res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml"+args.postfix)

    res = instance.loader.deploy(factory_path, caller=instance.caller, config="config.yml"+args.postfix)
    (factory, factory_eth, factory_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    print("factory", factory)
    print ("factory_eth", factory_eth.hex())
    print("factory_code", factory_code)
    erc20_filehash = get_filehash(factory, factory_code, factory_eth, instance.acc)
    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('create_erc20(bytes32)')
    receipt_list = []

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
                20000,
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

        receipt_list.append((str(erc20_id), erc20_ether, str(erc20_code), res["result"]))

    contracts = []
    event_error = 0
    receipt_error = 0
    total = 0
    for (erc20_id, erc20_ether, erc20_code, receipt) in receipt_list:
        total = total + 1
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)
        if res['result'] == None:
            receipt_error = receipt_error + 1
        else:
            try:
                check_address_event(res['result'], factory_eth, erc20_ether)
                contracts.append((erc20_id, erc20_ether.hex(), erc20_code))
            except AssertionError:
                event_error = event_error + 1

    with open(contracts_file+args.postfix, mode='w') as f:
        f.write(json.dumps(contracts))

    print("\ntotal:", total)
    print("event_error:", event_error)
    print("receipt_error:", receipt_error)


def mint_send(erc20_sol, erc20_eth_hex, erc20_code, payer_eth, payer_sol, acc, sum):

    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('mint(address,uint256)')
    trx_data = func_name + \
               bytes().fromhex("%024x" % 0 + payer_eth) + \
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
                AccountMeta(pubkey=payer_sol, is_signer=False, is_writable=True),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))
    res = client.send_transaction(trx, acc,
                                  opts=TxOpts(skip_confirmation=True, skip_preflight=True,
                                              preflight_commitment="confirmed"))
    return (erc20_eth_hex, payer_eth, res["result"])


def mint_create(accounts, acc, sum):

    with open(contracts_file+args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    receipt_list = []
    ia = iter(accounts)
    ic = iter(contracts)

    total = 0
    if args.scheme == "one-to-one":
        while total < args.count:
            print("mint ", total)
            try:
                (erc20_sol, erc20_eth_hex, erc20_code) = next(ic)
            except StopIteration as err:
                ic = iter(contracts)
                continue

            try:
                (payer_eth, payer_sol) = next(ia)
            except StopIteration as err:
                ia = iter(accounts)
                (payer_eth, payer_sol) = next(ia)

            receipt_list.append(mint_send(erc20_sol, erc20_eth_hex, erc20_code, payer_eth, payer_sol, acc, sum))
            total = total + 1
    else:
        for (erc20_sol, erc20_eth_hex, erc20_code) in contracts:
            for (payer_eth, payer_sol) in accounts:
                print("mint ", total)
                receipt_list.append(mint_send(erc20_sol, erc20_eth_hex, erc20_code, payer_eth, payer_sol, acc, sum))
                total = total + 1
                if total >= args.count:
                    return receipt_list
    return receipt_list


def mint_confirm(receipt_list, sum):
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    too_small_error = 0
    unknown_error = 0
    total = 0
    account_minted =[]

    for (erc20_eth_hex, acc_eth_hex, receipt) in receipt_list:
        total = total + 1
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)

        if res['result'] == None:
            receipt_error = receipt_error + 1
        else:
            if res['result']['meta']['err'] == None:
                if check_transfer_event(res['result'], erc20_eth_hex, bytes(20).hex(), acc_eth_hex, sum, b'\x11'):
                    account_minted.append(acc_eth_hex)
                    print("ok")
                else:
                    event_error = event_error + 1
            else:
                print(res['result'])
                found_nonce = False
                found_too_small = False
                for err in res['result']['meta']['err']['InstructionError']:
                    if err == "InvalidArgument":
                        found_nonce = True
                        break
                    if err == "AccountDataTooSmall":
                        found_too_small = True
                        break

                if found_nonce:
                    nonce_error = nonce_error + 1
                elif found_too_small:
                    too_small_error = too_small_error + 1
                else:
                    unknown_error = unknown_error + 1


    return (account_minted, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error)

def mint(accounts, acc):
    sum = 1000 * 10 ** 18
    receipt_list = mint_create(accounts, acc, sum)
    return mint_confirm(receipt_list, sum)


def create_accounts(args):
    instance = init_wallet()
    instance.init()

    receipt_list = {}
    for i in range(args.count):
        pr_key = w3.eth.account.from_key(os.urandom(32))
        acc_eth = bytes().fromhex(pr_key.address[2:])
        trx = Transaction()
        (transaction, acc_sol) = instance.loader.createEtherAccountTrx(acc_eth)
        trx.add(transaction)
        res = client.send_transaction(trx, instance.acc,
                                      opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed"))
        receipt_list[acc_eth.hex()] = (acc_sol, pr_key.privateKey.hex()[2:], res['result'])

    ether_accounts = []
    for (acc_eth_hex,  (acc_sol, pr_key_hex, receipt)) in receipt_list.items():
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)
        if res['result'] == None:
            print("createEtherAccount, get_confirmed_transaction() error")
        else:
            print(acc_eth_hex, acc_sol)
            ether_accounts.append((acc_eth_hex, acc_sol))

    # erc20.mint()
    (account_minted, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error) = mint(ether_accounts, instance.acc)

    to_file = []
    for acc_eth_hex in account_minted:
        (acc_sol, pr_key_hex, _) = receipt_list.get(acc_eth_hex)
        to_file.append((acc_eth_hex, pr_key_hex, acc_sol))

    print("\nmint total:", total)
    print("mint event_error:", event_error)
    print("mint receipt_error:", receipt_error)
    print("mint nonce_error:", nonce_error)
    print("mint unknown_error:", unknown_error)
    print("mint AccountDataTooSmall:", too_small_error)
    print("total accounts:", len(to_file))

    with open(accounts_file+args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))



def create_transactions(args):
    instance = init_wallet()
    instance.init()

    with open(contracts_file+args.postfix, mode='r') as f:
        contracts = json.loads(f.read())
    with open(accounts_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())
    transactions = open(transactions_file+args.postfix, mode='w')
    transactions = open(transactions_file+args.postfix, mode='a')

    func_name = abi.function_signature_to_4byte_selector('transfer(address,uint256)')
    total = 0
    if len(accounts) == 0:
        print ("accounts not found" )
        exit(1)

    if len(accounts) == 1:
        print ("accounts count too small" )
        exit(1)

    if len(contracts) == 0:
        print ("contracts not found")
        exit(1)

    ia = iter(accounts)
    ic = iter(contracts)

    while total < args.count:
        try:
            (erc20_sol, erc20_eth_hex, erc20_code) = next(ic)
        except StopIteration as err:
            ic = iter(contracts)
            continue
        try:
            (payer_eth, payer_prkey, payer_sol) = next(ia)
        except StopIteration as err:
            ia = iter(accounts)
            (payer_eth, payer_prkey, payer_sol) = next(ia)

        (receiver_eth, _, _) = accounts[random.randint(0, len(accounts)-1)]
        if payer_eth == receiver_eth:
            continue

        total = total + 1
        trx_data = func_name + \
                   bytes().fromhex("%024x" % 0 + receiver_eth) + \
                   bytes().fromhex("%064x" % transfer_sum)
        (from_addr, sign,  msg) = get_trx(
            bytes().fromhex(erc20_eth_hex),
            payer_sol,
            bytes().fromhex(payer_eth),
            trx_data,
            bytes.fromhex(payer_prkey)
        )
        trx = {}
        trx['from_addr'] = from_addr.hex()
        trx['sign'] = sign.hex()
        trx['msg']  = msg.hex()
        trx['erc20_sol'] = erc20_sol
        trx['erc20_eth'] = erc20_eth_hex
        trx['erc20_code'] = erc20_code
        trx['payer_sol'] = payer_sol
        trx['payer_eth'] = payer_eth
        trx['receiver_eth'] = receiver_eth

        transactions.write(json.dumps(trx)+"\n")

def get_block_hash():
    try:
        blockhash_resp = client.get_recent_blockhash()
        if not blockhash_resp["result"]:
            raise RuntimeError("failed to get recent blockhash")
        return (Blockhash(blockhash_resp["result"]["value"]["blockhash"]), time.time())
    except Exception as err:
        raise RuntimeError("failed to get recent blockhash") from err

def send_transactions(args):
    instance = init_wallet()
    instance.init()
    senders =init_senders()
    senders.init()
    count_err = 0

    eth_trx = open(transactions_file+args.postfix, mode='r')

    verify = open(verify_file+args.postfix, mode='w')
    verify = open(verify_file+args.postfix, mode='a')

    (recent_blockhash, blockhash_time) = get_block_hash()
    start = time.time()
    total = 0
    trx_times = []
    cycle_times = []
    for line in eth_trx:
        rec = json.loads(line)

        cycle_start = time.time()
        total = total + 1
        if args.count != None:
            if total > args.count:
                break
        if time.time() - blockhash_time > 5:
            (recent_blockhash, blockhash_time) = get_block_hash()

        from_addr = bytes.fromhex(rec['from_addr'])
        sign = bytes.fromhex(rec['sign'])
        msg = bytes.fromhex(rec['msg'])
        trx = Transaction()
        trx.add(sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
        trx.add(sol_instr_05((from_addr + sign + msg), rec['erc20_sol'], rec['erc20_code'], rec['payer_sol']))
        trx.recent_blockhash = recent_blockhash
        trx.sign(senders.next_acc())

        try:
            print("send trx", total)
            trx_start = time.time()
            res = client.send_raw_transaction(trx.serialize(),
                                              opts=TxOpts(skip_confirmation=True, preflight_commitment="confirmed", skip_preflight = True))
            trx_end = time.time()
        except Exception as err:
            print(err)
            count_err = count_err + 1
            continue
        verify.write(json.dumps((rec['erc20_eth'], rec['payer_eth'], rec['receiver_eth'], res["result"]))+"\n")
        cycle_end = time.time()
        trx_times.append(trx_end - trx_start)
        cycle_times.append(cycle_end - cycle_start)

    end = time.time()
    print("total:", total)
    print("errors:", count_err)
    print("time:", end-start, "sec" )
    print("avg send_raw_transaction time:  ", statistics.mean(trx_times), "sec")
    print("avg cycle time:                 ", statistics.mean(cycle_times), "sec")

def found_revert(res):
    if len(res['meta']['innerInstructions']) == 1:
        if len(res['meta']['innerInstructions'][0]['instructions']) == 1:
            ret_val = b58decode(res['meta']['innerInstructions'][0]['instructions'][0]['data'])
            if ret_val[:2].hex() == "06d0":
                return True
    return False


def verify_trx(args):
    verify = open(verify_file+args.postfix, 'r')
    total = 0
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    unknown_error = 0
    revert_error = 0

    for line in verify:
        total = total + 1
        if args.count != None:
            if total > args.count:
                break
        success = False
        (erc20_eth, payer_eth, receiver_eth, receipt) = json.loads(line)
        # confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)
        if res['result'] == None:
            receipt_error = receipt_error + 1
            print(success)
        else:
            if res['result']['meta']['err'] == None:
                if found_revert(res['result']):
                    revert_error = revert_error + 1
                    success = True
                else:
                    if check_transfer_event(res['result'], erc20_eth, payer_eth, receiver_eth, transfer_sum, b'\x12'):
                        success = True
                    else:
                        # print(res['result'])
                        event_error = event_error + 1
            else:
                print(res["result"])
                found = False
                for err in res['result']['meta']['err']['InstructionError']:
                    if err == "InvalidArgument":
                        found = True
                        break
                if found:
                    nonce_error = nonce_error + 1
                else:
                    unknown_error = unknown_error + 1
            print(success, res['result']['slot'])


    print("\ntotal:", total)
    print("event_error:", event_error)
    print("nonce_error:", nonce_error)
    print("unknown_error:", unknown_error)
    print("receipt_error:", receipt_error)
    print("revert_error:", revert_error)

def create_senders(args):
    total = 0
    receipt_list = []

    senders = open(senders_file+args.postfix, mode='w')
    senders = open(senders_file+args.postfix, mode='a')

    while total < args.count:
        total = total + 1
        acc = Account()
        tx = client.request_airdrop(acc.public_key(), 1000 * 10 ** 9, commitment=Confirmed)
        receipt_list.append((tx['result'], acc.secret_key(), acc.public_key()) )

    for (receipt, pr_key, pub_key ) in receipt_list:
        confirm_transaction(client, receipt)
        if getBalance(pub_key) == 0:
            print("request_airdrop error", str(pub_key))
            exit(0)
        line = pr_key.hex() + bytes(pub_key).hex()
        senders.write(line+"\n")


parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('--count', metavar="count of the transaction",  type=int,  help='count transaction (>=1)')
parser.add_argument('--step', metavar="step of the test", type=str,  help='deploy, create_senders, create_acc, create_trx, send_trx, '
                                                                           'veryfy_trx')
parser.add_argument('--scheme', metavar="(optional for stage=create_acc) scheme of the transactions", type=str,  help='one-to-one')
parser.add_argument('--postfix', metavar="filename postfix", type=str,  help='0,1,2..')

args = parser.parse_args()

if args.step == "deploy":
    deploy_contracts(args)
elif args.step == "create_acc":
    create_accounts(args)
elif args.step == "create_trx":
    create_transactions(args)
elif args.step == "send_trx":
    send_transactions(args)
elif args.step == "create_senders":
    create_senders(args)
elif args.step == "verify_trx":
    verify_trx(args)


