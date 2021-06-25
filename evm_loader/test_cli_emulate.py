import unittest
import solana
from eth_utils import abi
from web3.auto import w3
from solana_utils import *

tokenkeg = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
evm_loader_id = os.environ.get("EVM_LOADER")
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/ERC20/src")


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

    def transfer_ext(self, ether_caller, token, acc_from, acc_to, amount, signer):
        ether_trx = EthereumTransaction(
            ether_caller, self.contract_account, self.contract_code_account,
            abi.function_signature_to_4byte_selector('transferExt(uint256,uint256,uint256,uint256,uint256)')
            + bytes.fromhex(base58.b58decode(token).hex()
                            + base58.b58decode(acc_from).hex()
                            + base58.b58decode(acc_to).hex()
                            + "%064x" % amount
                            + signer.hex()
                            ),
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

        cls.token_acc1 = cls.spl_token.create_token_account(cls.token)
        print("token_acc1:", cls.token_acc1)

        cls.token_acc2 = cls.spl_token.create_token_account(cls.token, RandomAccount().get_path())
        print("token_acc2:", cls.token_acc2)

    def compare_tmpl_and_emulate_result(self, tmpl_json, emulate_result):
        print('tmpl_json:', json.dumps(tmpl_json, sort_keys=True, indent=2, separators=(',', ': ')))
        print('emulate_result:', json.dumps(emulate_result, sort_keys=True, indent=2, separators=(',', ': ')))

        self.assertEqual(tmpl_json["exit_status"], emulate_result["exit_status"])
        self.assertEqual(tmpl_json['result'], emulate_result['result'])

        left = map(lambda item: (item['account'], item['address'], item['contract']), tmpl_json['accounts'])
        right = map(lambda item: (item['account'], item['address'], item['contract']), emulate_result['accounts'])

        self.assertCountEqual(left, right)
        self.assertSetEqual(set(left), set(right))

    def test_successful_cli_emulate(self):
        balance1 = self.spl_token.balance(self.token_acc1)
        balance2 = self.spl_token.balance(self.token_acc2)
        mint_amount = 100
        self.spl_token.mint(self.token, self.token_acc1, mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        transfer_amount = 17
        (trx_data, result) \
            = self.contract.transfer_ext(self.ethereum_caller,
                                         self.token, self.token_acc1, self.token_acc2, transfer_amount * (10 ** 9),
                                         self.acc.public_key()._key)
        src_data = result['result']['meta']['innerInstructions'][-1]['instructions'][-1]['data']
        self.assertEqual(base58.b58decode(src_data)[0], 6)  # 6 means OnReturn
        self.assertLess(base58.b58decode(src_data)[1], 0xd0)  # less 0xd0 - success

        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + transfer_amount)

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               trx_data.hex())
        tmpl = """ {
            "accounts": [
                {
                    "account": "CONTRACT_ACCOUNT",
                    "address": "CONTRACT_ETHEREUM_ID",
                    "code_size": null,
                    "contract": "CONTRACT_CODE_ACCOUNT",
                    "new": false,
                    "writable": true
                },
                {
                    "account": "ETHEREUM_CALLER_ACCOUNT",
                    "address": "ETHEREUM_CALLER_ID",
                    "code_size": null,
                    "contract": null,
                    "new": false,
                    "writable": true
                }
            ],
            "exit_status": "succeed",
            "result": ""
        }"""
        tmpl = tmpl.replace('CONTRACT_ACCOUNT', self.contract.contract_account)
        tmpl = tmpl.replace('CONTRACT_ETHEREUM_ID', '0x' + self.contract.ethereum_id.hex())
        tmpl = tmpl.replace('CONTRACT_CODE_ACCOUNT', self.contract.contract_code_account)
        tmpl = tmpl.replace('ETHEREUM_CALLER_ACCOUNT', self.caller)
        tmpl = tmpl.replace('ETHEREUM_CALLER_ID', '0x' + self.ethereum_caller.hex())
        tmpl_json = json.loads(tmpl)

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)

        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount - transfer_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2 + transfer_amount)

    def test_unsuccessful_cli_emulate(self):
        balance1 = self.spl_token.balance(self.token_acc1)
        balance2 = self.spl_token.balance(self.token_acc2)
        mint_amount = 100
        self.spl_token.mint(self.token, self.token_acc1, mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)

        transfer_amount = self.spl_token.balance(self.token_acc1) + 1
        (trx_data, result)\
            = self.contract.transfer_ext(self.ethereum_caller,
                                         self.token, self.token_acc1, self.token_acc2, transfer_amount * (10 ** 9),
                                         self.acc.public_key()._key)
        self.assertEqual(result, None)

        emulate_result = emulate_external_call(self.ethereum_caller.hex(),
                                               self.contract.ethereum_id.hex(),
                                               trx_data.hex())
        tmpl = """ {
            "accounts": [
                {
                    "account": "CONTRACT_ACCOUNT",
                    "address": "CONTRACT_ETHEREUM_ID",
                    "code_size": null,
                    "contract": "CONTRACT_CODE_ACCOUNT",
                    "new": false,
                    "writable": true
                },
                {
                    "account": "ETHEREUM_CALLER_ACCOUNT",
                    "address": "ETHEREUM_CALLER_ID",
                    "code_size": null,
                    "contract": null,
                    "new": false,
                    "writable": true
                }
            ],
            "exit_status": "succeed",
            "result": ""
        }"""
        tmpl = tmpl.replace('CONTRACT_ACCOUNT', self.contract.contract_account)
        tmpl = tmpl.replace('CONTRACT_ETHEREUM_ID', '0x' + self.contract.ethereum_id.hex())
        tmpl = tmpl.replace('CONTRACT_CODE_ACCOUNT', self.contract.contract_code_account)
        tmpl = tmpl.replace('ETHEREUM_CALLER_ACCOUNT', self.caller)
        tmpl = tmpl.replace('ETHEREUM_CALLER_ID', '0x' + self.ethereum_caller.hex())
        tmpl_json = json.loads(tmpl)

        self.compare_tmpl_and_emulate_result(tmpl_json, emulate_result)

        # no changes after the emulation
        self.assertEqual(self.spl_token.balance(self.token_acc1), balance1 + mint_amount)
        self.assertEqual(self.spl_token.balance(self.token_acc2), balance2)


if __name__ == '__main__':
    unittest.main()
