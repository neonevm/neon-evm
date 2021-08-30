from tools import *

swap_contracts_file = "swap_contracts.json"
pair_file =  "contracts/uniswap/pair.bin"
user_tools_file = "contracts/uniswap/UserTools.binary"

factory_eth = "3ED1bc1418F305a530D41c764B44Bc6bb319DD03"
router_eth = "109CFeD64057CbF40bb26c02BEEBc9f090A08B0e"
weth_eth = "Fd91f022D16BE1B889f3d236Bcc2DaF80b92Cc4d"

def deploy_ctor_init(instance, src, dest, ctor_hex):
    ctor = bytearray().fromhex(ctor_hex)

    with open(src, mode='rb') as rbin:
        binary = rbin.read() + ctor
        with open(dest, mode='wb') as wbin:
            wbin.write(binary)
            res = instance.loader.deploy(dest, instance.caller)
            return (res['programId'], res['codeId'], bytes.fromhex(res['ethereum'][2:]))


def deploy_swap(args):

    instance = init_wallet()
    (weth_sol, _)  = instance.loader.ether2program(weth_eth)
    (factory_sol, _)  = instance.loader.ether2program(factory_eth)
    (router_sol, _)  = instance.loader.ether2program(router_eth)

    data = getAccountData(client, weth_sol, ACCOUNT_INFO_LAYOUT.sizeof())
    weth_code = PublicKey(ACCOUNT_INFO_LAYOUT.parse(data).code_acc)

    data = getAccountData(client, factory_sol, ACCOUNT_INFO_LAYOUT.sizeof())
    factory_code = PublicKey(ACCOUNT_INFO_LAYOUT.parse(data).code_acc)

    data = getAccountData(client, router_sol, ACCOUNT_INFO_LAYOUT.sizeof())
    router_code = PublicKey(ACCOUNT_INFO_LAYOUT.parse(data).code_acc)

    to_file = []
    to_file.append((weth_sol, weth_eth, str(weth_code)))
    to_file.append((factory_sol, factory_eth, str(factory_code)))
    to_file.append((router_sol, router_eth, str(router_code)))
    with open(swap_contracts_file + args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))

    print(" WETH:", weth_sol, weth_eth, weth_code)
    print(" FACTORY:", factory_sol, factory_eth, factory_code)
    print(" ROUTER", router_sol, router_eth, router_code)


def approve_send(erc20_sol, erc20_eth, erc20_code, msg_sender_sol, msg_sender_eth, msg_sender_prkey, router_eth,   acc, sum,):
    func_name = abi.function_signature_to_4byte_selector('approve(address,uint256)')
    input = func_name +  bytes().fromhex("%024x" % 0 + router_eth) + bytes().fromhex("%064x" % sum)

    (from_addr, sign, msg) = get_trx(
        bytes().fromhex(erc20_eth),
        msg_sender_sol,
        bytes().fromhex(msg_sender_eth),
        input,
        bytes.fromhex(msg_sender_prkey),
        0
    )
    trx = Transaction()
    trx.add(sol_instr_keccak(make_keccak_instruction_data(1, len(msg))))
    trx.add(sol_instr_05((from_addr + sign + msg), erc20_sol, erc20_code, msg_sender_sol))

    res = client.send_transaction(trx, acc,
                                  opts=TxOpts(skip_confirmation=True, skip_preflight=True,
                                              preflight_commitment="confirmed"))
    return res["result"]



def sol_instr_10_continue(meta, step_count):
    return TransactionInstruction(program_id=evm_loader_id,
                                  data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
                                  keys=meta)



def sol_instr_keccak(keccak_instruction):
    return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
        AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])


def create_storage_account(seed, acc):
    storage = PublicKey(
        sha256(bytes(acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
    print("Storage", storage)

    if getBalance(storage) == 0:
        trx = Transaction()
        trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10 ** 9, 128 * 1024,
                                      PublicKey(evm_loader_id)))
        send_transaction(client, trx, acc)

    return storage


def mint_and_approve_swap(args, accounts, sum, pr_key_list):
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    too_small_error = 0
    unknown_error = 0
    acc_and_tokens = []

    with open(contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    with open(swap_contracts_file + args.postfix, mode='r') as f:
        swap_contracts = json.loads(f.read())
    (router_sol, router_eth, router_code) = swap_contracts[2]
    print(router_sol, router_eth, router_code)

    senders = init_senders(args)

    receipt_list = []
    ia = iter(accounts)
    ic = iter(contracts)

    total = 0
    while total < args.count:
        print("mint ", total)

        try:
            (token_a_sol, token_a_eth, token_a_code) = next(ic)
        except StopIteration as err:
            ic = iter(contracts)
            (token_a_sol, token_a_eth, token_a_code) = next(ic)

        try:
            (token_b_sol, token_b_eth, token_b_code) = next(ic)
        except StopIteration as err:
            ic = iter(contracts)
            (token_b_sol, token_b_eth, token_b_code) = next(ic)

        try:
            (account_eth, account_sol) = next(ia)
        except StopIteration as err:
            ia = iter(accounts)
            (account_eth, account_sol) = next(ia)
        (_, account_prkey) = pr_key_list.get(account_eth)

        one_acc_receipts = []
        acc = senders.next_acc()

        receipt = mint_erc20_send(token_a_sol, token_a_code, account_eth, account_sol, acc, sum)
        one_acc_receipts.append((token_a_eth, bytes(20).hex(), account_eth, receipt))

        receipt = mint_erc20_send(token_b_sol, token_b_code, account_eth, account_sol, acc, sum)
        one_acc_receipts.append((token_b_eth, bytes(20).hex(), account_eth, receipt))

        receipt = approve_send(token_a_sol, token_a_eth, token_a_code, account_sol, account_eth, account_prkey, router_eth, acc, sum)
        one_acc_receipts.append((token_a_eth, account_eth, router_eth, receipt))

        receipt = approve_send(token_b_sol, token_b_eth, token_b_code, account_sol, account_eth, account_prkey, router_eth, acc, sum)
        one_acc_receipts.append((token_b_eth, account_eth, router_eth, receipt))

        receipt_list.append(
            (one_acc_receipts, token_a_sol, token_a_eth, token_a_code, token_b_sol, token_b_eth, token_b_code)
        )

        total = total + 1
        if total % 100 == 0 or total == args.count:
            for (one_acc_receipts, token_a_sol, token_a_eth, token_a_code, token_b_sol, token_b_eth, token_b_code) in receipt_list:
                cnt = 0
                confirmed = []
                for (erc20_eth_hex, msg_sender, to, receipt) in one_acc_receipts:
                    if cnt < 2:
                        (confirmed_, event_error_, receipt_error_, nonce_error_, unknown_error_, too_small_error_) = \
                            mint_or_approve_confirm([(erc20_eth_hex, msg_sender, to, receipt)], sum, "Transfer")
                    else:
                        (confirmed_, event_error_, receipt_error_, nonce_error_, unknown_error_, too_small_error_) =  \
                            mint_or_approve_confirm([(erc20_eth_hex, msg_sender, to, receipt)], sum, "Approval")
                    cnt = cnt + 1
                    confirmed = confirmed + confirmed_
                    event_error = event_error + event_error_
                    receipt_error = receipt_error + receipt_error_
                    nonce_error = nonce_error + nonce_error_
                    unknown_error = unknown_error + unknown_error_
                    too_small_error = too_small_error + too_small_error_

                if len(confirmed) == 4:  # all transactions of the account is successful
                    item = (confirmed[0], token_a_sol, token_a_eth, token_a_code, token_b_sol, token_b_eth, token_b_code)
                    acc_and_tokens.append(item)
            receipt_list = []


    return (acc_and_tokens, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error)


def get_salt(tool_sol, tool_code, tool_eth, token_a, token_b, acc):
    input = bytearray.fromhex("03") + \
            abi.function_signature_to_4byte_selector('get_salt(address,address)') + \
            bytes().fromhex("%024x" % 0 + token_a) + \
            bytes().fromhex("%024x" % 0 + token_b)

    trx = Transaction()
    trx.add(
        TransactionInstruction(
            program_id=evm_loader_id,
            data=input,
            keys=[
                AccountMeta(pubkey=tool_sol, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(tool_sol), ETH_TOKEN_MINT_ID), is_signer=False,
                            is_writable=True),
                AccountMeta(pubkey=tool_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
            ]))
    result = send_transaction(client, trx, acc)['result']
    # print(result)
    if result['meta']['err'] != None:
        print(result)
        print("Error: result['meta']['err'] != None")
        exit(1)

    if result == None:
        print("Error: result == None")
        exit(1)

    assert (result['meta']['err'] == None)
    assert (len(result['meta']['innerInstructions']) == 1)
    assert (len(result['meta']['innerInstructions'][0]['instructions']) == 2)
    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    assert (data[:1] == b'\x06')  # OnReturn
    assert (data[1] == 0x11)  # 11 - Machine encountered an explict stop

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    assert (data[:1] == b'\x07')  # 7 means OnEvent
    assert (data[1:21] == tool_eth)
    assert (data[21:29] == bytes().fromhex('%016x' % 1)[::-1])  # topics len
    hash = data[61:93]
    return hash


def create_account_with_seed (acc, seed, storage_size):
    account = accountWithSeed(acc.public_key(), seed, PublicKey(evm_loader_id))
    print("HOLDER ACCOUNT:", account)
    if getBalance(account) == 0:
        trx = Transaction()
        trx.add(createAccountWithSeed(acc.public_key(), acc.public_key(), seed, 10 ** 9, storage_size,
                                      PublicKey(evm_loader_id)))
        send_transaction(client, trx, acc)
    return account


def write_layout(offset, data):
    return (bytes.fromhex("00000000")+
            offset.to_bytes(4, byteorder="little")+
            len(data).to_bytes(8, byteorder="little")+
            data)


def write_trx_to_holder_account(acc, holder, sign, unsigned_msg):
    msg = sign + len(unsigned_msg).to_bytes(8, byteorder="little") + unsigned_msg

    # Write transaction to transaction holder account
    offset = 0
    receipts = []
    rest = msg
    while len(rest):
        (part, rest) = (rest[:1000], rest[1000:])
        trx = Transaction()
        # logger.debug("sender_sol %s %s %s", sender_sol, holder, acc.public_key())
        trx.add(TransactionInstruction(program_id=evm_loader_id,
                                       data=write_layout(offset, part),
                                       keys=[
                                           AccountMeta(pubkey=holder, is_signer=False, is_writable=True),
                                           AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                                       ]))
        receipts.append(client.send_transaction(trx, acc, opts=TxOpts(skip_confirmation=True, preflight_commitment=Confirmed))["result"])
        offset += len(part)
    print("receipts %s", receipts)
    for rcpt in receipts:
        confirm_transaction(client, rcpt)
        print("confirmed: %s", rcpt)

    return holder


def create_pair(tools_sol, tools_code, tools_eth, token_a_eth, token_b_eth, instance):
    with open(pair_file, mode='rb') as f:
        hash = Web3.keccak(f.read())
    salt = get_salt(tools_sol, tools_code, tools_eth, token_a_eth, token_b_eth, instance.acc)

    pair_eth = bytes(Web3.keccak(b'\xff' + bytes.fromhex(factory_eth) + salt + hash)[-20:])
    (pair_sol, _) = instance.loader.ether2program(pair_eth)

    if getBalance(pair_sol) == 0:
        seed = b58encode(bytes.fromhex(pair_eth.hex()))
        pair_code = accountWithSeed(instance.acc.public_key(), str(seed, 'utf8'), PublicKey(evm_loader_id))
    else:
        data = getAccountData(client, pair_sol, ACCOUNT_INFO_LAYOUT.sizeof())
        pair_code = PublicKey(ACCOUNT_INFO_LAYOUT.parse(data).code_acc)
    print("\npair_info.code_acc",pair_code, "\n")



    # (pair_code, _) = instance.loader.ether2seed(pair_eth)
    print("")
    print("pair_sol", pair_sol)
    print("pair_eth", pair_eth.hex())
    print("pair_code", pair_code)
    print("")

    trx = Transaction()
    if getBalance(pair_code) == 0:
        trx.add(
            createAccountWithSeed(
                instance.acc.public_key(),
                instance.acc.public_key(),
                str(seed, 'utf8'),
                10 ** 9,
                20000,
                PublicKey(evm_loader_id))
        )
    if getBalance(pair_sol) == 0:
        trx.add(instance.loader.createEtherAccountTrx(pair_eth, code_acc=pair_code)[0])

    if len(trx.instructions):
        res = send_transaction(client, trx, instance.acc)

    return (pair_sol, pair_eth, pair_code)


def add_liquidity(args):
    instance = init_wallet()
    senders = init_senders(args)

    res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml"+args.postfix)

    with open(swap_contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    (weth_sol, weth_eth, weth_code) = contracts[0]
    (factory_sol, factory_eth, factory_code)= contracts[1]
    (router_sol, router_eth, router_code) = contracts[2]

    print(" WETH:", weth_sol, weth_eth, weth_code)
    print(" FACTORY:", factory_sol, factory_eth, factory_code)
    print(" ROUTER", router_sol, router_eth, router_code)

    res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml" + args.postfix)
    res = instance.loader.deploy(user_tools_file, caller=instance.caller, config="config.yml" + args.postfix)

    (tools_sol, tools_eth, tools_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    holder = create_account_with_seed(instance.acc, os.urandom(5).hex(), 128 * 1024)

    with open(accounts_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())

    total = 0
    ok  = 0
    func_name = abi.function_signature_to_4byte_selector('addLiquidity(address,address,uint256,uint256,uint256,uint256,address,uint256)')

    sum = 10**18
    to_file = []

    for (msg_sender_eth, msg_sender_prkey, msg_sender_sol, token_a_sol, token_a_eth, token_a_code, token_b_sol, token_b_eth, token_b_code) in accounts:
        print (" ")
        print (" token_a_eth",token_a_eth)
        print (" token_b_eth",token_b_eth)
        if total >= args.count:
            break
        total = total + 1
        input = func_name + \
                   bytes().fromhex("%024x" % 0 + token_a_eth) + \
                   bytes().fromhex("%024x" % 0 + token_b_eth) + \
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) + \
                   bytes().fromhex("%024x" % 0 + msg_sender_eth) + \
                   bytes().fromhex("%064x" % 10**18)

        (from_addr, sign, msg) = get_trx(
            bytes().fromhex(router_eth),
            msg_sender_sol,
            bytes().fromhex(msg_sender_eth),
            input,
            bytes.fromhex(msg_sender_prkey),
            0)

        acc = senders.next_acc()
        storage = create_storage_account(os.urandom(5).hex(), acc)

        print("WRITE TO HOLDER ACCOUNT")
        write_trx_to_holder_account(instance.acc, holder, sign, msg)

        (pair_sol, pair_eth, pair_code) = create_pair(
            tools_sol, tools_code, tools_eth, token_a_eth, token_b_eth, instance)

        meta = [
            AccountMeta(pubkey=holder, is_signer=False, is_writable=True),
            AccountMeta(pubkey=storage, is_signer=False, is_writable=True),

            AccountMeta(pubkey=router_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(router_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
            AccountMeta(pubkey=router_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=msg_sender_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(msg_sender_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

            AccountMeta(pubkey=token_a_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(token_a_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
            AccountMeta(pubkey=token_a_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=token_b_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(token_b_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=token_b_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=factory_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(factory_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=factory_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=pair_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(pair_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=pair_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
        ]

        print("Begin", total)
        step = 0
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=evm_loader_id, data=bytearray.fromhex("0B") + step.to_bytes(8, byteorder="little"), keys=meta))
        print("ExecuteTrxFromAccountDataIterative:")
        res = send_transaction(client, trx, instance.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(sol_instr_10_continue(meta[1:], 1000))
            res = send_transaction(client, trx, instance.acc)
            result = res["result"]

            print(result)
            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    print("ok")
                    ok = ok + 1
                    to_file.append((msg_sender_eth, msg_sender_prkey, msg_sender_sol,
                                    token_a_sol, token_a_eth, token_a_code,
                                    token_b_sol, token_b_eth, token_b_code,
                                    str(pair_sol), pair_eth.hex(), str(pair_code)))
                    break;

    print("total", total)
    print("success", ok)
    with open(liquidity_file + args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))


def create_transactions_swap(args):
    instance = init_wallet()
    senders = init_senders(args)

    with open(swap_contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    (weth_sol, weth_eth, weth_code) = contracts[0]
    (factory_sol, factory_eth, factory_code)= contracts[1]
    (router_sol, router_eth, router_code) = contracts[2]

    print(" WETH:", weth_sol, weth_eth, weth_code)
    print(" FACTORY:", factory_sol, factory_eth, factory_code)
    print(" ROUTER", router_sol, router_eth, router_code)


    with open(liquidity_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())

    total = 0
    ok  = 0
    func_name = abi.function_signature_to_4byte_selector('swapExactTokensForTokens(uint256,uint256,address[],address,uint256)')

    holder = create_account_with_seed(instance.acc, os.urandom(5).hex(), 128 * 1024)

    sum = 10**18
    for (msg_sender_eth, msg_sender_prkey, msg_sender_sol, token_a_sol, token_a_eth, token_a_code, token_b_sol, token_b_eth, token_b_code,
         pair_sol, pair_eth, pair_code) in accounts:
        if total >= args.count:
            break
        total = total + 1
        input = func_name + \
                bytes().fromhex("%064x" % sum) +\
                bytes().fromhex("%064x" % 0) +\
                bytes().fromhex("%064x" % 0xa0) +\
                bytes().fromhex("%024x" % 0 + msg_sender_eth) + \
                bytes().fromhex("%064x" % 10**18) + \
                bytes().fromhex("%064x" % 2) + \
                bytes().fromhex("%024x" % 0 + token_a_eth) + \
                bytes().fromhex("%024x" % 0 + token_b_eth)
        print("")
        print("input:", input.hex())
        print("")

        (from_addr, sign, msg) = get_trx(
            bytes().fromhex(router_eth),
            msg_sender_sol,
            bytes().fromhex(msg_sender_eth),
            input,
            bytes.fromhex(msg_sender_prkey),
            0)

        acc = senders.next_acc()
        storage = create_storage_account(os.urandom(5).hex(), acc)
        print("WRITE TO HOLDER ACCOUNT")
        write_trx_to_holder_account(instance.acc, holder, sign, msg)

        meta = [
            AccountMeta(pubkey=holder, is_signer=False, is_writable=True),
            AccountMeta(pubkey=storage, is_signer=False, is_writable=True),

            AccountMeta(pubkey=router_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(router_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
            AccountMeta(pubkey=router_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=msg_sender_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(msg_sender_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),

            AccountMeta(pubkey=token_a_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(token_a_sol), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
            AccountMeta(pubkey=token_a_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=token_b_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(token_b_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=token_b_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=factory_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(factory_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=factory_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=pair_sol, is_signer=False, is_writable=True),
            AccountMeta(pubkey=get_associated_token_address(PublicKey(pair_sol), ETH_TOKEN_MINT_ID), is_signer=False,is_writable=True),
            AccountMeta(pubkey=pair_code, is_signer=False, is_writable=True),

            AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
            AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
        ]

        instruction = from_addr + sign + msg
        print("Begin", total)
        step = 0
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=evm_loader_id, data=bytearray.fromhex("0B") + step.to_bytes(8, byteorder="little"), keys=meta))
        print("ExecuteTrxFromAccountDataIterative:")
        res = send_transaction(client, trx, instance.acc)

        while (True):
            print("Continue")
            trx = Transaction()
            trx.add(sol_instr_10_continue(meta[1:], 1000))
            res = send_transaction(client, trx, instance.acc)
            result = res["result"]

            print(result)
            if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
                data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
                if (data[0] == 6):
                    print("ok")
                    ok = ok + 1
                    break;
    print("total", total)
    print("success", ok)


