from enum import Enum
from time import sleep

import spl.token.client
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from web3.auto import w3
from web3 import Web3
import argparse
from eth_utils import abi
from base58 import b58decode
import random
from solana.blockhash import *
import statistics
from spl.token.constants import TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
from spl.token.instructions import get_associated_token_address
from spl.token.client import *
from spl.token._layouts import INSTRUCTIONS_LAYOUT, InstructionType  # type: ignore


evm_loader_id = os.environ.get("EVM_LOADER")
trx_cnt = os.environ.get("CNT", 10)

chain_id = 111
transfer_sum = 1

sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
contracts_file = "contract.json"
accounts_file = "account.json"
liquidity_file = "liquidity.json"
transactions_file = "transaction.json"
senders_file = "sender.json"
verify_file = "verify.json"
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))

trx_count = {}

class transfer_type(Enum):
    erc20 = 0,
    spl = 1,
    uniswap = 2

class init_senders():
    def __init__(cls, args):
        cls.accounts = []
        file = open(senders_file + args.postfix, mode='r')
        for line in file:
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
    def __init__(cls):
        print("\ntest_performance.py init")
        cls.token = SplToken(solana_url)

        # wallet = RandomAccount()
        wallet = OperatorAccount()

        if getBalance(wallet.get_acc().public_key()) == 0:
            tx = client.request_airdrop(wallet.get_acc().public_key(), 1000000 * 10 ** 9, commitment=Confirmed)
            confirm_transaction(client, tx["result"])

        regular_wallet = WalletAccount(wallet_path())
        cls.regular_acc = regular_wallet.get_acc()
        # cls.wallet_token = cls.token.create_token_account(ETH_TOKEN_MINT_ID, owner=wallet.get_path())
        cls.wallet_token = get_associated_token_address(PublicKey(wallet.get_acc().public_key()), ETH_TOKEN_MINT_ID)
        cls.token.mint(ETH_TOKEN_MINT_ID, cls.wallet_token, 10000)


        assert (getBalance(wallet.get_acc().public_key()) > 0)

        cls.loader = EvmLoader(wallet, evm_loader_id)
        cls.acc = wallet.get_acc()
        cls.keypath = wallet.get_path()

        # Create ethereum account for user account
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



def check_approve_event(result, erc20_eth, acc_from, acc_to, sum, return_code):
    # assert(result['meta']['err'] == None)

    if (len(result['meta']['innerInstructions']) != 1):
        print("check event Approval")
        print("len(result['meta']['innerInstructions']) != 1", len(result['meta']['innerInstructions']))
        return False

    if (len(result['meta']['innerInstructions'][0]['instructions']) != 2):
        print("check event Approval")
        print("len(result['meta']['innerInstructions'][0]['instructions']) != 2",
              len(result['meta']['innerInstructions'][0]['instructions']))
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    if (data[:1] != b'\x06'):  #  OnReturn
        print("check event Approval")
        print("data[:1] != x06", data[:1].hex())
        return False

    if(data[1:2] != return_code):    # 11 - Machine encountered an explict stop,  # 12 - Machine encountered an explict return
        print("check event Approval")
        print("data[1:2] != return_code", data[1:2].hex(), return_code.hex())
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    if(data[:1] != b'\x07'):  # 7 means OnEvent
        print("check event Approval")
        print("data[:1] != x07", data[:1].hex())
        return  False


    if (data[1:21] != bytes.fromhex(erc20_eth)):
        print("check event Approval")
        print("data[1:21] != bytes.fromhex(erc20_eth)", data[1:21].hex(), erc20_eth)
        return False

    if(data[21:29] != bytes().fromhex('%016x' % 3)[::-1]):  # topics len
        print("check event Approval")
        print("data[21:29] != bytes().fromhex('%016x' % 3)[::-1]", data[21:29].hex())
        return False

    if(data[29:61] != abi.event_signature_to_log_topic('Approval(address,address,uint256)')):  # topics
        print("check event Approval")
        print("data[29:61] != abi.event_signature_to_log_topic('Approval(address,address,uint256)')",
              data[29:61].hex(),
              abi.event_signature_to_log_topic('Approval(address,address,uint256)').hex())
        return False

    if (data[61:93] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_from)):
        print(result)
        print("check event Approval")
        print("data[61:93] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)",
              data[61:93].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)).hex())
        return False

    if(data[93:125] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_to)):  # from
        print("check event Approval")
        print("data[93:125] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)",
              data[93:125].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)).hex()
              )
        return False

    if (data[125:157] != bytes().fromhex("%064x" % sum)):  # value
        print("check event Approval")
        print("data[125:157] != bytes().fromhex('%064x' % sum)",
              data[125:157].hex(),
              '%064x' % sum)
        return False

    return True


def check_transfer_event(result, erc20_eth, acc_from, acc_to, sum, return_code):
    # assert(result['meta']['err'] == None)

    if (len(result['meta']['innerInstructions']) != 1):
        print("check event Transfer")
        print("len(result['meta']['innerInstructions']) != 1", len(result['meta']['innerInstructions']))
        return False

    if (len(result['meta']['innerInstructions'][0]['instructions']) != 2):
        print("check event Transfer")
        print(result)
        print("len(result['meta']['innerInstructions'][0]['instructions']) != 2",
              len(result['meta']['innerInstructions'][0]['instructions']))
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
    if (data[:1] != b'\x06'):  #  OnReturn
        print("check event Transfer")
        print("data[:1] != x06", data[:1].hex())
        return False

    if(data[1:2] != return_code):    # 11 - Machine encountered an explict stop,  # 12 - Machine encountered an explict return
        print("check event Transfer")
        print("data[1:2] != return_code", data[1:2].hex(), return_code.hex())
        return False

    data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
    if(data[:1] != b'\x07'):  # 7 means OnEvent
        print("check event Transfer")
        print("data[:1] != x07", data[:1].hex())
        return  False


    if (data[1:21] != bytes.fromhex(erc20_eth)):
        print("check event Transfer")
        print("data[1:21] != bytes.fromhex(erc20_eth)", data[1:21].hex(), erc20_eth)
        return False

    if(data[21:29] != bytes().fromhex('%016x' % 3)[::-1]):  # topics len
        print("check event Transfer")
        print("data[21:29] != bytes().fromhex('%016x' % 3)[::-1]", data[21:29].hex())
        return False

    if(data[29:61] != abi.event_signature_to_log_topic('Transfer(address,address,uint256)')):  # topics
        print("check event Transfer")
        print("data[29:61] != abi.event_signature_to_log_topic('Transfer(address,address,uint256)')",
              data[29:61].hex(),
              abi.event_signature_to_log_topic('Transfer(address,address,uint256)').hex())
        return False

    if (data[61:93] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_from)):
        print("check event Transfer")
        print("data[61:93] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)",
              data[61:93].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_from)).hex())
        return False

    if(data[93:125] != bytes().fromhex("%024x" % 0) + bytes.fromhex(acc_to)):  # from
        print("check event Transfer")
        print("data[93:125] != bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)",
              data[93:125].hex(),
              (bytes().fromhex('%024x' % 0) + bytes.fromhex(acc_to)).hex()
              )
        return False

    if (data[125:157] != bytes().fromhex("%064x" % sum)):  # value
        print("check event Transfer")
        print("data[125:157] != bytes().fromhex('%064x' % sum)",
              data[125:157].hex(),
              '%064x' % sum)
        return False

    return True


def sol_instr_keccak(keccak_instruction):
    return TransactionInstruction(
        program_id=keccakprog,
        data=keccak_instruction,
        keys=[AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False)]
    )


def sol_instr_05(evm_instruction, contract, contract_code, caller):
    account_meta = [
        AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
        AccountMeta(pubkey=get_associated_token_address(PublicKey(contract), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
        AccountMeta(pubkey=caller, is_signer=False, is_writable=True),
        AccountMeta(pubkey=get_associated_token_address(PublicKey(caller), ETH_TOKEN_MINT_ID), is_signer=False, is_writable=True),
        AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
        AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
        AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
        # AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
        AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
    ]
    if contract_code != "":
        account_meta.insert(2, AccountMeta(pubkey=contract_code, is_signer=False, is_writable=True))

    return TransactionInstruction(program_id=evm_loader_id,
                                  data=bytearray.fromhex("05") + evm_instruction,
                                  keys=account_meta)


def mint_erc20_send(erc20_sol, erc20_code, account_eth, account_sol, acc, sum):
    func_name = bytearray.fromhex("03") + abi.function_signature_to_4byte_selector('mint(address,uint256)')

    trx_data = func_name + \
               bytes().fromhex("%024x" % 0 + account_eth) + \
               bytes().fromhex("%064x" % sum)
    trx = Transaction()
    trx.add(
        TransactionInstruction(
            program_id=evm_loader_id,
            data=trx_data,
            keys=[
                AccountMeta(pubkey=erc20_sol, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(erc20_sol), ETH_TOKEN_MINT_ID),
                            is_signer=False, is_writable=True),
                AccountMeta(pubkey=erc20_code, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc.public_key(), is_signer=True, is_writable=False),
                AccountMeta(pubkey=account_sol, is_signer=False, is_writable=True),
                AccountMeta(pubkey=get_associated_token_address(PublicKey(account_sol), ETH_TOKEN_MINT_ID),
                            is_signer=False, is_writable=True),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey=ETH_TOKEN_MINT_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=TOKEN_PROGRAM_ID, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False)
            ]))
    res = client.send_transaction(trx, acc,
                                  opts=TxOpts(skip_confirmation=True, skip_preflight=True,
                                              preflight_commitment="confirmed"))
    return  res["result"]


def mint_or_approve_confirm(receipt_list, sum, event):
    event_error = 0
    receipt_error = 0
    nonce_error = 0
    too_small_error = 0
    unknown_error = 0
    account_confirmed =[]

    for (erc20_eth_hex, acc_from, acc_to, receipt) in receipt_list:
        confirm_transaction(client, receipt)
        res = client.get_confirmed_transaction(receipt)

        if res['result'] == None:
            receipt_error = receipt_error + 1
        else:
            if res['result']['meta']['err'] == None:
                if event == "Transfer":
                    if check_transfer_event(res['result'], erc20_eth_hex, acc_from, acc_to, sum, b'\x11'):
                        account_confirmed.append(acc_to)
                        print("ok")
                    else:
                        event_error = event_error + 1
                elif event == "Approval":
                    if check_approve_event(res['result'], erc20_eth_hex, acc_from, acc_to,  sum, b'\x12'):
                        account_confirmed.append(acc_from)
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

    return (account_confirmed, event_error, receipt_error, nonce_error, unknown_error, too_small_error)


def found_revert(res):
    if len(res['meta']['innerInstructions']) == 1:
        if len(res['meta']['innerInstructions'][0]['instructions']) == 1:
            ret_val = b58decode(res['meta']['innerInstructions'][0]['instructions'][0]['data'])
            if ret_val[:2].hex() == "06d0":
                return True
    return False


def get_acc(accounts, ia):
    try:
        (payer_eth, payer_prkey, payer_sol) = next(ia)
    except StopIteration as err:
        ia = iter(accounts)
        (payer_eth, payer_prkey, payer_sol) = next(ia)
    return (payer_eth, payer_prkey, payer_sol)


def get_trx(contract_eth, caller, caller_eth, input, pr_key, value, use_local_nonce_counter=True):
    if trx_count.get(caller) != None and use_local_nonce_counter:
        trx_count[caller] = trx_count[caller] + 1
    else:
        trx_count[caller] = getTransactionCount(client, caller)

    tx = {'to': contract_eth, 'value': value, 'gas': 9999999999, 'gasPrice': 1,
        'nonce': trx_count[caller], 'data': input, 'chainId': chain_id}
    (from_addr, sign, msg) = make_instruction_data_from_tx(tx, pr_key)

    assert (from_addr == caller_eth)
    return (from_addr, sign, msg)



