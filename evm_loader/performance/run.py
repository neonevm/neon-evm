from spl_ import *
from uniswap import *
from erc20 import *


parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('--count', metavar="count of the transaction",  type=int,  help='count transaction (>=1)', default=trx_cnt)
parser.add_argument('--step', metavar="step of the test", type=str,
                    help= ' For ERC20.transfers: deploy, create_senders, create_acc, create_trx, veryfy_trx.'
                          ' For spl-token transfers: create_senders, create_acc_spl, create_trx_spl, verify_trx_spl'
                          ' For uniswap-interface swap operations: deploy, create_senders, create_acc, add_liquidity, create_trx, verify_trx.')
parser.add_argument('--postfix', metavar="filename postfix", type=str,  help='0,1,2..', default='')

args = parser.parse_args()

if args.step == "deploy":
    deploy_contracts(args)
elif args.step == "create_acc":
    create_accounts(args)
elif args.step == "create_acc_spl":
    create_accounts(args, transfer_type.spl)
elif args.step == "create_trx":
    create_transactions(args)
elif args.step == "create_trx_spl":
    create_transactions_spl(args)
elif args.step == "send_trx":
    send_transactions(args)
elif args.step == "create_senders":
    create_senders(args)
elif args.step == "verify_trx":
    verify_trx(args)
elif args.step == "verify_trx_spl":
    verify_trx_spl(args)
elif args.step == "add_liquidity":
    add_liquidity(args)
elif args.step == "deploy_uniswap":
    add_liquidity(args)


