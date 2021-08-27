from tools import *

# weth9_path = "contracts/uniswap/WETH9.binary"
# factory_path_src = "contracts/uniswap/UniswapV2Factory.bin"
# factory_path_dest = "contracts/uniswap/UniswapV2Factory.binary"
# router02_path_src = "contracts/uniswap/UniswapV2Router02.bin"
# router02_path_dest = "contracts/uniswap/UniswapV2Router02.binary"

approve_file = "approve.json"
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



def approve_send(erc20_sol, erc20_eth_hex, erc20_code, account_eth, account_sol, acc, sum):
    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('approve(address,uint256)')
    return erc20_method_call(erc20_sol, erc20_eth_hex, erc20_code, account_eth, account_sol, acc, sum, func_name)


def approve_confirm(receipt_list, sum, from_acc):
    return erc20_method_call_confirm(receipt_list, sum, "Approve", from_acc)


def approve(args, sum):
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    too_small_error = 0
    unknown_error = 0
    account_approved = []

    instance = init_wallet()

    with open(contracts_file + args.postfix, mode='r') as f:
        contracts = json.loads(f.read())
    with open(accounts_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())

    receipt_list = []
    ia = iter(accounts)
    ic = iter(contracts)

    pr_key_list = {}
    total = 0
    approve_trx_caller_eth = solana2ether(instance.acc.public_key())
    while total < args.count:
        print("approve ", total)
        try:
            (erc20_sol, erc20_eth_hex, erc20_code) = next(ic)
        except StopIteration as err:
            ic = iter(contracts)
            continue

        try:
            (account_eth, account_prkey, account_sol) = next(ia)
        except StopIteration as err:
            ia = iter(accounts)
            (account_eth, account_prkey, account_sol) = next(ia)

        pr_key_list[account_eth] = (account_prkey, account_sol)

        receipt_list.append(approve_send(erc20_sol, erc20_eth_hex, erc20_code, account_eth, account_sol, instance.acc, sum))

        if total % 500 == 0 or total == args.count - 1:
            (account_approved_, event_error_, receipt_error_, nonce_error_, unknown_error_,
             too_small_error_) = approve_confirm(receipt_list, sum, approve_trx_caller_eth.hex())

            account_approved = account_approved + account_approved_
            event_error = event_error + event_error_
            receipt_error = receipt_error + receipt_error_
            nonce_error = nonce_error + nonce_error_
            unknown_error = unknown_error + unknown_error_
            too_small_error = too_small_error + too_small_error_
            receipt_list = []
        total = total + 1

    approved = []
    for account_eth_hex in account_approved:
        (pr_key_hex, account_sol) = pr_key_list.get(account_eth_hex)
        approved.append((account_eth_hex, pr_key_hex, account_sol ))

    return (approved, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error)


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

    (account_approved, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error) = approve(args, 1000 * 10 ** 18)

    to_file = []
    for (account_eth_hex, pr_key_hex, account_sol) in account_approved:
        to_file.append((account_eth_hex, pr_key_hex, account_sol))

    print("\napprove total:", total)
    print("approve event_error:", event_error)
    print("approve receipt_error:", receipt_error)
    print("approve nonce_error:", nonce_error)
    print("approve unknown_error:", unknown_error)
    print("approve AccountDataTooSmall:", too_small_error)
    print("total accounts:", len(to_file))

    with open(approve_file + args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))