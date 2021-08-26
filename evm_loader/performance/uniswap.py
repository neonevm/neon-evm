from tools import *



def deploy_uniswap(args):
    instance = init_wallet()

    res = solana_cli().call("config set --keypair " + instance.keypath + " -C config.yml" + args.postfix)

    res = instance.loader.deploy(router02_path, caller=instance.caller, config="config.yml" + args.postfix)
    (router02, router02_eth, router02_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    print("router2", router02)
    print("router2_eth", router02_eth.hex())
    print("router2_code", router02_code)

    to_file = []
    to_file.append((router02, router02_eth.hex(), router02_code))
    with open(router02_file + args.postfix, mode='w') as f:
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

    res = instance.loader.deploy(router02_path, caller=instance.caller, config="config.yml"+args.postfix)
    (router02, router02_eth, router02_code) = (res['programId'], bytes.fromhex(res['ethereum'][2:]), res['codeId'])

    print("router2", router02)
    print("router2_eth", router02_eth.hex())
    print("router2_code", router02_code)

    to_file = []
    to_file.append((router02, router02_eth.hex(), router02_code))
    with open(router02_file + args.postfix, mode='w') as f:
        f.write(json.dumps(to_file))

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