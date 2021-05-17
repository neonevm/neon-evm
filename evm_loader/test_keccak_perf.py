from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx
from eth_utils import abi

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
evm_loader_id = os.environ.get("EVM_LOADER")


class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(solana_url, wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

    def sol_instr_syscall(self):
        return TransactionInstruction(program_id=self.loader.loader_id, data=bytearray.fromhex("b1"), keys=[AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),])

    def sol_instr_sha3(self):
        return TransactionInstruction(program_id=self.loader.loader_id, data=bytearray.fromhex("b2"), keys=[AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),])

    def test_syscall(self):
        trx = Transaction()
        trx.add(self.sol_instr_syscall())
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]["meta"]["logMessages"][3]
        print("Keccak Syscall")
        print(result)

    def test_sha3_lib(self):
        trx = Transaction()
        trx.add(self.sol_instr_sha3())
        result = http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]["meta"]["logMessages"][3]
        print("SHA3 lib call")
        print(result)

if __name__ == '__main__':
    unittest.main()
