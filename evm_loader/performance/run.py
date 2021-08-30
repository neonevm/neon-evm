from spl_ import *
from uniswap import *
from erc20 import *


parser = argparse.ArgumentParser(description='Process some integers.')
parser.add_argument('--count', metavar="count of the transaction",  type=int,  help='count transaction (>=1)', default=trx_cnt)
parser.add_argument('--step', metavar="step of the test", type=str,
                    help= ' For ERC20.transfers: deploy_erc20, create_senders, create_acc, create_trx, veryfy_trx.'
                          ' For spl-token transfers: create_senders, create_acc, create_trx, verify_trx'
                          ' For swap operations: deploy_erc20, deploy_swap, create_senders, create_acc, add_liquidity, create_trx.')
parser.add_argument('--postfix', metavar="filename postfix", type=str,  help='0,1,2..', default='')
parser.add_argument('--type', metavar="transfer type", type=str,  help='erc20, spl, swap', default='erc20')

args = parser.parse_args()

if args.step == "deploy_erc20":
    deploy_erc20(args)
elif args.step == "create_acc":
    create_accounts(args)
elif args.step == "create_trx":
    if args.type == "spl":
        create_transactions_spl(args)
    elif args.type == "erc20":
        create_transactions(args)
    elif args.type == "swap":
        create_transactions_swap(args)
elif args.step == "send_trx":
    send_transactions(args)
elif args.step == "create_senders":
    create_senders(args)
elif args.step == "verify_trx":
    if args.type == "spl":
        verify_trx_spl(args)
    else:
        verify_trx(args)
elif args.step == "add_liquidity":
    add_liquidity(args)
elif args.step == "deploy_swap":
    deploy_swap(args)


