from tools import *

# weth9_path = "contracts/uniswap/WETH9.binary"
# factory_path_src = "contracts/uniswap/UniswapV2Factory.bin"
# factory_path_dest = "contracts/uniswap/UniswapV2Factory.binary"
# router02_path_src = "contracts/uniswap/UniswapV2Router02.bin"
# router02_path_dest = "contracts/uniswap/UniswapV2Router02.binary"

uniswap_contracts_file = "uniswap_contracts.json"

factory_eth = "9D6A7a98721437Ae59D4b8253e80eBc642196d56"
router_eth = "DeF2f37003e4FFeF6B94C6fb4961f0dCc97f15cA"
weth_eth = "50dbC82D76409D19544d6ca95D844633E222aC71"

def deploy_ctor_init(instance, src, dest, ctor_hex):
    ctor = bytearray().fromhex(ctor_hex)

    with open(src, mode='rb') as rbin:
        binary = rbin.read() + ctor
        with open(dest, mode='wb') as wbin:
            wbin.write(binary)
            res = instance.loader.deploy(dest, instance.caller)
            return (res['programId'], res['codeId'], bytes.fromhex(res['ethereum'][2:]))

def deploy_uniswap(args):


    instance = init_wallet()
    (weth_sol, _)  = instance.loader.ether2program(weth_eth)
    (factory_sol, _)  = instance.loader.ether2program(factory_eth)
    (router_sol, _)  = instance.loader.ether2program(router_eth)

    weth_code = instance.loader.ether2seed(weth_eth)[0]
    factory_code = instance.loader.ether2seed(factory_eth)[0]
    router_code = instance.loader.ether2seed(router_eth)[0]

    # res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml" + args.postfix)

    # # deploy WETH
    # res = instance.loader.deploy(weth9_path, caller=instance.caller, config="config.yml" + args.postfix)
    # (weth9, weth9_eth, weth9_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])
    # print("weth9", weth9)
    # print("weth9_eth", weth9_eth.hex())
    # print("weth9_code", weth9_code)
    #
    # res = instance.loader.deploy(router02_path_dest, caller=instance.caller, config="config.yml" + args.postfix)
    # (weth9, weth9_eth, weth9_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])
    # print("weth9", weth9)
    # print("weth9_eth", weth9_eth.hex())
    # print("weth9_code", weth9_code)
    #
    # return;

    # # deploy Factory
    # ctor_hex =str("%024x" % 0) + instance.caller_ether.hex()
    # print("ctor_hex", ctor_hex)
    # with open(factory_path_src, mode='r') as r:
    #     content = r.read() + ctor_hex
    #     bin = bytearray().fromhex(content)
    #     with open(factory_path_dest, mode='wb') as w:
    #         w.write(bin)
    #         res = instance.loader.deploy(factory_path_dest, caller=instance.caller, config="config.yml" + args.postfix)
    #         (factory, factory_eth, factory_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])
    #
    #         print("factory", factory)
    #         print("factory_eth", factory_eth.hex())
    #         print("factory_code", factory_code)

    # deploy Router02
    #
    # factory_eth = bytes().fromhex("c03a0611c7df00c760343b0752d6c572667ebb90")
    # weth9_eth = bytes().fromhex("c03a0611c7df00c760343b0752d6c572667ebb90")
    # ctor_hex = str("%024x" % 0) + factory_eth.hex() + str("%024x" % 0) + weth9_eth.hex()
    # print("ctor_hex", ctor_hex)
    # with open(router02_path_src, mode='rb') as rbin:
    #     binary = rbin.read() + bytes().fromhex(ctor_hex)
    #     with open(router02_path_dest, mode='wb') as wbin:
    #         wbin.write(binary)
    #         res = instance.loader.deploy(router02_path_dest, caller=instance.caller, config="config.yml" + args.postfix)
    #         (router02, router02_eth, router02_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])
    #
    #         print("router02", router02)
    #         print("router02_eth", router02_eth.hex())
    #         print("router02_code", router02_code)

    to_file = []
    to_file.append((weth_sol, weth_eth, str(weth_code)))
    to_file.append((factory_sol, factory_eth, str(factory_code)))
    to_file.append((router_sol, router_eth, str(router_code)))
    with open(uniswap_contracts_file + args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))


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


def sol_instr_09_partial_call(storage_account, step_count, evm_instruction, instance):
    return TransactionInstruction(program_id=instance.loader.loader_id,
                                  data=bytearray.fromhex("09") + step_count.to_bytes(8,
                                                                                     byteorder='little') + evm_instruction,
                                  keys=[
                                      AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=instance.reId, is_signer=False, is_writable=True),
                                      AccountMeta(
                                          pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID),
                                          is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller),
                                                                                      ETH_TOKEN_MINT_ID),
                                                  is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                  ])


def sol_instr_10_continue(storage_account, step_count, instance):
    return TransactionInstruction(program_id=self.loader.loader_id,
                                  data=bytearray.fromhex("0A") + step_count.to_bytes(8, byteorder='little'),
                                  keys=[
                                      AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                      AccountMeta(
                                          pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID),
                                          is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller),
                                                                                      ETH_TOKEN_MINT_ID),
                                                  is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                  ])


def sol_instr_12_cancel(self, storage_account):
    return TransactionInstruction(program_id=self.loader.loader_id,
                                  data=bytearray.fromhex("0C"),
                                  keys=[
                                      AccountMeta(pubkey=storage_account, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                      AccountMeta(
                                          pubkey=get_associated_token_address(PublicKey(self.reId), ETH_TOKEN_MINT_ID),
                                          is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.re_code, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=get_associated_token_address(PublicKey(self.caller),
                                                                                      ETH_TOKEN_MINT_ID),
                                                  is_signer=False, is_writable=True),
                                      AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
                                      AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                  ])


def call_begin(storage, steps, msg, instruction, instance):
    print("Begin")
    trx = Transaction()
    trx.add(sol_instr_keccak(make_keccak_instruction_data(1, len(msg), 9)))
    trx.add(sol_instr_09_partial_call(storage, steps, instruction, instance))
    return send_transaction(client, trx, instance.acc)


def call_continue(storage, steps, instance):
    print("Continue")
    trx = Transaction()
    trx.add(sol_instr_10_continue(storage, steps, instance))
    return send_transaction(client, trx, instance.acc)



def get_call_parameters(self, input, instance):
    tx = {'to': solana2ether(self.reId), 'value': 0, 'gas': 99999999, 'gasPrice': 1,
          'nonce': getTransactionCount(client, instance.caller), 'data': input, 'chainId': 111}
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, instance.acc.secret_key())
    assert (from_addr == instance.caller_ether)

    return (from_addr, sign, msg)


def sol_instr_keccak(keccak_instruction):
    return TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
        AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])


def create_storage_account(seed, instance):
    storage = PublicKey(
        sha256(bytes(instance.acc.public_key()) + bytes(seed, 'utf8') + bytes(PublicKey(evm_loader_id))).digest())
    print("Storage", storage)

    if getBalance(storage) == 0:
        trx = Transaction()
        trx.add(createAccountWithSeed(instance.acc.public_key(), instance.acc.public_key(), seed, 10 ** 9, 128 * 1024,
                                      PublicKey(evm_loader_id)))
        send_transaction(client, trx, instance.acc)

    return storage


def call_partial_signed(input, instance):
    (from_addr, sign, msg) = get_call_parameters(input, instance)
    instruction = from_addr + sign + msg

    storage = create_storage_account(sign[:8].hex(), instance)
    call_begin(storage, 10, msg, instruction, instance)

    while (True):
        result = call_continue(storage, 50, instance)["result"]

        if (result['meta']['innerInstructions'] and result['meta']['innerInstructions'][0]['instructions']):
            data = b58decode(result['meta']['innerInstructions'][0]['instructions'][-1]['data'])
            if (data[0] == 6):
                return result


# def add_liquidity_call(tokenA_eth, tokenB_eth, caller_eth, sum, to):
#
#     trx = Transaction()
#     trx.add(
#         TransactionInstruction(
#             program_id=evm_loader_id,
#             data=trx_data,
#             keys=[
#                 AccountMeta(pubkey=erc20_sol, is_signer=False, is_writable=True),
#                 AccountMeta(pubkey=get_associated_token_address(PublicKey(erc20_sol), ETH_TOKEN_MINT_ID),
#                             is_signer=False, is_writable=True),
#                 AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
#                 AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
#                 AccountMeta(pubkey=account_sol, is_signer=False, is_writable=True),
#                 AccountMeta(pubkey=get_associated_token_address(PublicKey(account_sol), ETH_TOKEN_MINT_ID),
#                             is_signer=False, is_writable=True),
#                 AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
#                 AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
#                 AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
#                 AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False)
#             ]))
#     res = client.send_transaction(trx, acc,
#                                   opts=TxOpts(skip_confirmation=True, skip_preflight=True,
#                                               preflight_commitment="confirmed"))
#     return (erc20_eth_hex, account_eth, res["result"])


def add_liquidity(args):
    instance = init_wallet()

    res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml"+args.postfix)

    # res = instance.loader.deploy(router02_path, caller=instance.caller, config="config.yml"+args.postfix)
    # (router02, router02_eth, router02_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    # print("router2", router02)
    # print("router2_eth", router02_eth.hex())
    # print("router2_code", router02_code)

    with open(uniswap_contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    (weth_sol, weth_eth, weth_code) = contracts[0]
    (factory_sol, factory_eth, factory_code)= contracts[1]
    (router_sol, router_eth, router_code) = contracts[2]

    print(weth_sol, weth_eth, weth_code)
    print(factory_sol, factory_eth, factory_code)
    print(router_sol, router_eth, router_code)


    with open(accounts_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())

    total = 0
    func_name = abi.function_signature_to_4byte_selector('addLiquidity(address,address,uint256,uint256,uint256,uint256,address,uint256)')
    ia = iter(accounts)
    while total < args.count:
        (token_a, token_a_eth, token_a_code) = get_acc(accounts, ia)
        (token_b, token_b_eth, token_b_code) = get_acc(accounts, ia)

        input = func_name + \
                   bytes().fromhex("%024x" % 0 + token_a_eth) + \
                   bytes().fromhex("%024x" % 0 + token_b_eth) + \
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) +\
                   bytes().fromhex("%064x" % sum) + \
                   bytes().fromhex("%024x" % 0 + instance.caller_ether.hex()) + \
                   bytes().fromhex("%064x" % 10*18)

        call_partial_signed(input, instance)

        break;



def mint_and_approve_swap(args, accounts, sum, pr_key_list):
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    too_small_error = 0
    unknown_error = 0
    acc_and_tokens = []

    with open(contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())

    with open(uniswap_contracts_file + args.postfix, mode='r') as f:
        uniswap_contracts = json.loads(f.read())
    (router_sol, router_eth, router_code) = uniswap_contracts[2]
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

