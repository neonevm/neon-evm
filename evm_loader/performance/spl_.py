from tools import  *

def mint_spl(accounts, instance):

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
                                      opts=TxOpts(skip_confirmation=True, skip_preflight=True,
                                                  preflight_commitment="confirmed"))
        receipt_list.append((acc_eth_hex, res["result"]))

        total = total + 1
        if total % 50 == 0 or total == len(accounts):
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


    accounts = []
    with open(accounts_file+args.postfix, mode='r') as f:
        for line in f:
            accounts.append(line)

    with open(transactions_file + args.postfix, mode='w') as f:

        total = 0
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


def create_account_spl(args):
    instance = init_wallet()

    receipt_list = []
    total = 0
    confirmed = 0
    error = 0

    with open(accounts_file + args.postfix, mode='a') as f:
        while confirmed < args.count*2:

            pr_key = w3.eth.account.from_key(os.urandom(32))
            trx = Transaction()
            (transaction, acc_sol) = instance.loader.createEtherAccountTrx(bytes().fromhex(pr_key.address[2:]))
            trx.add(transaction)

            param = spl_token.TransferParams(
                program_id = TOKEN_PROGRAM_ID,
                source = instance.wallet_token,
                dest = get_associated_token_address(PublicKey(acc_sol), ETH_TOKEN_MINT_ID),
                owner = instance.acc.public_key(),
                amount=10**9
            )
            trx.add(spl_token.transfer(param))

            res = client.send_transaction(trx, instance.acc,
                                          opts=TxOpts(skip_confirmation=True, skip_preflight=True, preflight_commitment="confirmed"))
            receipt_list.append((res['result'], pr_key.address[2:], pr_key.privateKey.hex()[2:], acc_sol))

            total = total + 1
            if total % 50 == 0 :
                for (receipt, address, pr_key,  acc_sol) in receipt_list:
                    try:
                        confirm_transaction_(client, receipt)
                        res = client.get_confirmed_transaction(receipt)
                        if res['result'] == None:
                            print("receipt is empty", receipt)
                            error = error + 1
                        else:
                            line = {}
                            line['address'] = address
                            line['pr_key'] = pr_key
                            line['account'] = acc_sol
                            f.write(json.dumps(line) + "\n")

                            confirmed = confirmed + 1;
                    except:
                        print(f"transaction is lost {receipt}")

                receipt_list = []

    print("\nconfirmed:", confirmed)
    print("receipt error:", error)
    print("total:", total)

