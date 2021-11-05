from tools import  *

def mint_spl(accounts, key, instance):
    if key == '':
        print("args.key is empty")
        exit(1)

    print(key)
    wallet = OperatorAccount(key).get_acc()


    receipt_list = []
    total = 0
    receipt_error = 0
    account_minted = []
    for (acc_eth_hex, acc_sol) in accounts:
        dest = get_associated_token_address(PublicKey(acc_sol), ETH_TOKEN_MINT_ID)
        print("mint: ", dest)

        param = spl_token.TransferParams(
            program_id = TOKEN_PROGRAM_ID,
            source = instance.wallet_token,
            dest = dest,
            owner = instance.acc.public_key(),
            amount=10**9
        )
        trx = Transaction()
        trx.add(spl_token.transfer(param))

        res = client.send_transaction(trx, instance.acc,
                                      opts=TxOpts(skip_confirmation=True, skip_preflight=False,
                                                  preflight_commitment="confirmed"))
        receipt_list.append((acc_eth_hex, res["result"]))

        total = total + 1
        if total % 100 == 0 or total == len(accounts):
            for (acc_eth_hex, receipt) in receipt_list:
                confirm_transaction(client, receipt)
                res = client.get_confirmed_transaction(receipt)
                if res['result'] == None:
                    receipt_error = receipt_error + 1
                    print(res['result'])
                else:
                    account_minted.append(acc_eth_hex)
            receipt_list = []


    event_error = 0
    nonce_error = 0
    unknown_error = 0
    too_small_error = 0

    return (account_minted, total, event_error, receipt_error, nonce_error, unknown_error, too_small_error)


def verify_trx_spl(args):
    verify = open(verify_file+args.postfix, 'r')
    total = 0
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
    print("nonce_error:", nonce_error)
    print("unknown_error:", unknown_error)
    print("receipt_error:", receipt_error)
    print("revert_error:", revert_error)



def create_transactions_spl(args):
    # instance = init_wallet()

    with open(accounts_file+args.postfix, mode='r') as f:
        accounts = json.loads(f.read())

    with open(transactions_file + args.postfix, mode='w') as f:

        total = 0
        if len(accounts) == 0:
            print ("accounts not found" )
            exit(1)

        if len(accounts) == 1:
            print ("accounts count too small" )
            exit(1)

        ia = iter(accounts)

        while total < args.count:
            (payer_eth, payer_prkey, payer_sol) = get_acc(accounts, ia)
            (receiver_eth, _, receiver_sol) = get_acc(accounts, ia)

            total = total + 1
            (from_addr, sign,  msg) = get_trx(
                bytes().fromhex(receiver_eth),
                payer_sol,
                bytes().fromhex(payer_eth),
                "",
                bytes.fromhex(payer_prkey),
                transfer_sum*10**9
            )
            trx = {}
            trx['from_addr'] = from_addr.hex()
            trx['sign'] = sign.hex()
            trx['msg']  = msg.hex()
            trx['erc20_sol'] = receiver_sol
            trx['erc20_eth'] = receiver_eth
            trx['erc20_code'] = ""
            trx['payer_sol'] = payer_sol
            trx['payer_eth'] = payer_eth
            trx['receiver_eth'] = receiver_eth

            f.write(json.dumps(trx)+"\n")
