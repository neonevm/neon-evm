import unittest
import solana
from eth_utils import abi
from solana.blockhash import Blockhash
from web3.auto import w3
from solana_utils import *
from solana.rpc.commitment import Commitment, Confirmed

tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/ERC20/src")


def create_ether_trx_data(token, acc_from, acc_to, amount, owner):
    trx_data = abi.function_signature_to_4byte_selector('transferExt(uint256,uint256,uint256,uint256,uint256)') \
               + bytes.fromhex(base58.b58decode(token).hex()
                               + base58.b58decode(acc_from).hex()
                               + base58.b58decode(acc_to).hex()
                               + "%064x" % amount
                               + owner.hex()
                               )

    return trx_data


class ExternalCall:
    """Encapsulate the all data of the ExternalCall ethereum contract."""

    def __init__(self, contract_account, contract_code_account, ethereum_id=None):
        self.contract_account = contract_account
        self.contract_code_account = contract_code_account
        print("contract_id:", self.contract_account)
        print("contract_code:", self.contract_code_account)
        if ethereum_id is not None:
            self.ethereum_id = ethereum_id
            print("contract_ethereum:_id", self.ethereum_id.hex())
        self.neon_evm_client = None

    def set_neon_evm_client(self, neon_evm_client):
        self.neon_evm_client = neon_evm_client

    def transfer_ext(self, ether_caller, token, acc_from, acc_to, amount, owner):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            create_ether_trx_data(token, acc_from, acc_to, amount, owner),
            [
                AccountMeta(pubkey=acc_from, is_signer=False, is_writable=True),
                AccountMeta(pubkey=acc_to, is_signer=False, is_writable=True),
                AccountMeta(pubkey=token, is_signer=False, is_writable=False),
                AccountMeta(pubkey=PublicKey(tokenkeg), is_signer=False, is_writable=False),
            ])

        result = None
        try:
            result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
            print(result)
        except solana.rpc.api.SendTransactionError as err:
            import sys
            print("ERR: transfer_ext: {}".format(err))
        return ether_trx.trx_data, result

    def call(self, ether_caller, trx_data, account_metas):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account, trx_data, account_metas)
        result = None
        try:
            result = self.neon_evm_client.send_ethereum_trx_single(ether_trx)
            print(result)
        except solana.rpc.api.SendTransactionError as err:
            import sys
            print("ERR: transfer_ext: {}".format(err))
        return result


def emulate_external_call(sender, contract, trx_data):
    print('\nEmulate external call:')
    print('sender:', sender)
    print('contract:', contract)
    print('trx_data:', trx_data)
    cli = neon_cli()
    cli_result = cli.emulate(evm_loader_id, sender + ' ' + contract + ' ' + trx_data)
    print('cli_result:', cli_result)
    emulate_result = json.loads(cli_result)
    print('emulate_result:', emulate_result)
    return emulate_result


def emulate_external_call_by_rpc(contract, trx_data, signer, pubkey):
    print('\nEmulate external call by rpc:')
    print('contract:', contract)
    print('trx_data:', trx_data)
    print('signer:', signer)
    blockhash = Blockhash(client.get_recent_blockhash(Confirmed)["result"]["value"]["blockhash"])
    print('blockhash:', blockhash)
    trx = Transaction(recent_blockhash=blockhash)
    trx.add(TransactionInstruction(program_id=PublicKey(contract),
                                   data=trx_data,
                                   keys=[AccountMeta(pubkey=PublicKey(contract), is_signer=False, is_writable=False)]))
    trx.sign(signer)

    try:
        trx.serialize()
    except Exception as err:
        print("trx.serialize() exception")
        if str(err).startswith("transaction too large:"):
            raise Exception("transaction too large")
        raise

    response = client.simulate_transaction(trx, commitment=Confirmed)
    print('response:', response)
    return response


class EmulateTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.wallet = WalletAccount(wallet_path())
        cls.acc = cls.wallet.get_acc()
        cls.loader = EvmLoader(cls.wallet, evm_loader_id)
        cls.spl_token = SplToken(solana_url)

        cls.neon_evm_client = NeonEvmClient(cls.acc, cls.loader)
        cls.neon_evm_client.set_execute_mode(ExecuteMode.ITERATIVE)

        # Create ethereum account for user account
        cls.caller_eth_pr_key = w3.eth.account.from_key(cls.acc.secret_key())
        cls.ethereum_caller = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.ethereum_caller)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            cls.loader.createEtherAccount(cls.ethereum_caller)
            print("Done\n")

        print('Account: {} ({})'.format(cls.acc.public_key(), bytes(cls.acc.public_key()).hex()))
        print('Ethereum Caller: {}-{}'.format(cls.ethereum_caller.hex(), cls.caller_nonce))
        print('Solana Caller: {} ({})'.format(cls.caller, bytes(PublicKey(cls.caller)).hex()))

        res = cls.loader.deploy(CONTRACTS_DIR + "ExternalCall.binary", cls.caller)
        cls.contract = ExternalCall(res['programId'], res['codeId'], bytes.fromhex(res['ethereum'][2:]))
        cls.contract.set_neon_evm_client(cls.neon_evm_client)

        cls.token = cls.spl_token.create_token()
        print("token:", cls.token)

        cls.wallet1 = RandomAccount()
        cls.token_acc1 = cls.spl_token.create_token_account(cls.token, cls.wallet1.get_path())
        print("token_acc1:", cls.token_acc1)

        cls.wallet2 = RandomAccount()
        cls.token_acc2 = cls.spl_token.create_token_account(cls.token, cls.wallet2.get_path())
        print("token_acc2:", cls.token_acc2)

    def compare_accounts(self, left_json, right_json):
        left = map(lambda item: (item['account'], item['address'], item['contract']), left_json['accounts'])
        right = map(lambda item: (item['account'], item['address'], item['contract']), right_json['accounts'])

        self.assertCountEqual(left, right)
        self.assertSetEqual(set(left), set(right))

    def compare_solana_accounts(self, left_json, right_json):
        left = map(lambda item: (item['pubkey'], item['is_signer'], item['is_writable']), left_json['solana_accounts'])
        right = map(lambda item: (item['pubkey'], item['is_signer'], item['is_writable']),
                    right_json['solana_accounts'])

        self.assertCountEqual(left, right)
        self.assertSetEqual(set(left), set(right))

    def compare_tmpl_and_emulate_result(self, tmpl_json, emulate_result):
        print('tmpl_json:', json.dumps(tmpl_json, sort_keys=True, indent=2, separators=(',', ': ')))
        print('emulate_result:', json.dumps(emulate_result, sort_keys=True, indent=2, separators=(',', ': ')))

        self.assertEqual(tmpl_json["exit_status"], emulate_result["exit_status"])
        self.assertEqual(tmpl_json['result'], emulate_result['result'])

        self.compare_accounts(tmpl_json, emulate_result)
        self.compare_solana_accounts(tmpl_json, emulate_result)

    # @unittest.skip("a.i.")
    def test_successful_cli_emulate(self):
        print('\n---------------------------')
        print('test_successful_cli_emulate')
        balance1 = self.spl_token.balance(self.token_acc1)
        balance2 = self.spl_token.balance(self.token_acc2)
        mint_amount = 100
        self.spl_token.mint(self.token, self.token_acc1, mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        owner_token_acc1 = self.wallet1.get_acc().public_key()

        ether_trx_data = create_ether_trx_data(self.token, self.token_acc1, self.token_acc2,
                                               11 * (10 ** 9), bytes(self.acc.public_key()))

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               ether_trx_data.hex())
        print('\n::::::::::::::::::::::::::::::')
        print('emulate_result:', json.dumps(emulate_result, sort_keys=True, indent=2, separators=(',', ': ')))

        transfer_amount = 17
        (trx_data, result) \
            = self.contract.transfer_ext(self.ethereum_caller,
                                         self.token, self.token_acc1, self.token_acc2, transfer_amount * (10 ** 9),
                                         bytes(self.acc.public_key()),
                                         bytes(owner_token_acc1))
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        self.assertEqual(base58.b58decode(src_data)[0], 6)  # 6 means OnReturn
        self.assertLess(base58.b58decode(src_data)[1], 0xd0)  # less 0xd0 - success

        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + transfer_amount)

        tmpl_json = {
            "solana_accounts": [
                {
                    "pubkey": str(tokenkeg),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token_acc1),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.token_acc2),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.acc.public_key()),
                    "is_signer": True,
                    "is_writable": False,
                },
            ],
            "accounts": [
                {
                    "account": str(self.contract.contract_account),
                    "address": '0x' + self.contract.ethereum_id.hex(),
                    "code_size": None,
                    "contract": str(self.contract.contract_code_account),
                    "new": False,
                    "writable": True
                },
                {
                    "account": str(self.caller),
                    "address": '0x' + self.ethereum_caller.hex(),
                    "code_size": None,
                    "contract": None,
                    "new": False,
                    "writable": True
                }
            ],
            "exit_status": "succeed",
            "result": ""
        }

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               trx_data.hex())

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)

        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + transfer_amount)

        tmpl_json = {
            "solana_accounts": [
                {
                    "pubkey": str(tokenkeg),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token_acc1),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.token_acc2),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.acc.public_key()),
                    "is_signer": True,
                    "is_writable": False,
                },
            ],
            "accounts": [
                {
                    "account": str(self.contract.contract_account),
                    "address": '0x' + self.contract.ethereum_id.hex(),
                    "code_size": None,
                    "contract": str(self.contract.contract_code_account),
                    "new": False,
                    "writable": True,
                },
                {
                    "account": str(self.caller),
                    "address": '0x' + self.ethereum_caller.hex(),
                    "code_size": None,
                    "contract": None,
                    "new": False,
                    "writable": True,
                },
            ],
            "exit_status": "succeed",
            "result": ""
        }

        ether_trx_data = create_ether_trx_data(self.token, self.token_acc1, self.token_acc2,
                                               transfer_amount * (10 ** 9),
                                               bytes(owner_token_acc1))

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               ether_trx_data.hex())

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)
        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + transfer_amount)

        solana_accounts = [AccountMeta(pubkey=item['pubkey'],
                                       is_signer=item['is_signer'],
                                       is_writable=item['is_writable'])
                           for item in emulate_result['solana_accounts']]

        print('solana_accounts:', solana_accounts)

        result = self.contract.call(self.ethereum_caller, trx_data, solana_accounts)

        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        self.assertEqual(base58.b58decode(src_data)[0], 6)  # 6 means OnReturn
        self.assertLess(base58.b58decode(src_data)[1], 0xd0)  # less 0xd0 - success

        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - 2 * transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + 2 * transfer_amount)

    @unittest.skip("a.i.")
    def test_unsuccessful_cli_emulate(self):
        print('\n-----------------------------')
        print('test_unsuccessful_cli_emulate')
        balance1 = self.spl_token.balance(self.token_acc1)
        balance2 = self.spl_token.balance(self.token_acc2)
        mint_amount = 100
        self.spl_token.mint(self.token, self.token_acc1, mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        transfer_amount = self.spl_token.balance(self.token_acc1) + 1
        (trx_data, result) \
            = self.contract.transfer_ext(self.ethereum_caller,
                                         self.token, self.token_acc1, self.token_acc2, transfer_amount * (10 ** 9),
                                         bytes(self.acc.public_key()))
        self.assertEqual(result, None)

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               trx_data.hex())
        tmpl_json = {
            "solana_accounts": [
                {
                    "pubkey": str(tokenkeg),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token_acc1),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.token_acc2),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.acc.public_key()),
                    "is_signer": True,
                    "is_writable": False,
                },
            ],
            "accounts": [
                {
                    "account": str(self.contract.contract_account),
                    "address": '0x' + self.contract.ethereum_id.hex(),
                    "code_size": None,
                    "contract": str(self.contract.contract_code_account),
                    "new": False,
                    "writable": True
                },
                {
                    "account": str(self.caller),
                    "address": '0x' + self.ethereum_caller.hex(),
                    "code_size": None,
                    "contract": None,
                    "new": False,
                    "writable": True
                }
            ],
            "exit_status": "succeed",
            "result": ""
        }

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)

        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        tmpl_json = {
            "solana_accounts": [
                {
                    "pubkey": str(tokenkeg),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token),
                    "is_signer": False,
                    "is_writable": False,
                },
                {
                    "pubkey": str(self.token_acc1),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.token_acc2),
                    "is_signer": False,
                    "is_writable": True,
                },
                {
                    "pubkey": str(self.acc.public_key()),
                    "is_signer": True,
                    "is_writable": False,
                },
            ],
            "accounts": [
                {
                    "account": str(self.contract.contract_account),
                    "address": '0x' + self.contract.ethereum_id.hex(),
                    "code_size": None,
                    "contract": str(self.contract.contract_code_account),
                    "new": False,
                    "writable": True,
                },
                {
                    "account": str(self.caller),
                    "address": '0x' + self.ethereum_caller.hex(),
                    "code_size": None,
                    "contract": None,
                    "new": False,
                    "writable": True,
                },
            ],
            "exit_status": "succeed",
            "result": ""
        }

        ether_trx_data = create_ether_trx_data(self.token, self.token_acc1, self.token_acc2,
                                               transfer_amount * (10 ** 9), bytes(self.acc.public_key()))

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               ether_trx_data.hex())

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)
        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        solana_accounts = [AccountMeta(pubkey=item['pubkey'],
                                       is_signer=item['is_signer'],
                                       is_writable=item['is_writable'])
                           for item in emulate_result['solana_accounts']]

        print('solana_accounts:', solana_accounts)

        result = self.contract.call(self.ethereum_caller, trx_data, solana_accounts)

        self.assertEqual(result, None)

        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

    @unittest.skip("a.i.")
    def test_emulate_external_call_by_rpc(self):
        print('\n---------------------------------')
        print('test_emulate_external_call_by_rpc')
        balance1 = self.spl_token.balance(self.token_acc1)
        balance2 = self.spl_token.balance(self.token_acc2)
        mint_amount = 100
        self.spl_token.mint(self.token, self.token_acc1, mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        transfer_amount = 31
        ether_trx_data = create_ether_trx_data(self.token, self.token_acc1, self.token_acc2,
                                               transfer_amount * (10 ** 9), bytes(self.acc.public_key()))

        print('ether_trx_data:', ether_trx_data.hex())

        blockhash = Blockhash(client.get_recent_blockhash(Confirmed)["result"]["value"]["blockhash"])
        print('blockhash:', blockhash)
        trx = Transaction(recent_blockhash=blockhash)
        trx.add(TransactionInstruction(program_id=PublicKey(tokenkeg),
                                       data=ether_trx_data.hex(),
                                       keys=[
                                           AccountMeta(pubkey=self.token, is_signer=False, is_writable=False),
                                           AccountMeta(pubkey=self.token_acc1, is_signer=False, is_writable=False),
                                           AccountMeta(pubkey=self.token_acc2, is_signer=False, is_writable=False),
                                           AccountMeta(pubkey=PublicKey(tokenkeg), is_signer=False, is_writable=False),
                                       ]))
        # trx.sign(self.acc)
        trx.serialize()
        response = client.simulate_transaction(trx, commitment=Confirmed)
        print('response:', response)
        self.assertGreater(len(response), 0)


if __name__ == '__main__':
    unittest.main()
