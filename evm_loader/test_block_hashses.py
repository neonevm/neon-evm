import unittest
import base58
from solana_utils import *

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
evm_loader_id = os.environ.get("EVM_LOADER")


class DeployTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_deploy.py setUpClass")

        operator_wallet = OperatorAccount(operator1_keypair_path())
        cls.operator_acc = operator_wallet.get_acc()


    def get_blocks_from_neonevm(self, count):
        slot_hash = {}
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=evm_loader_id,
            data=bytes.fromhex('f0') + count.to_bytes(4, byteorder='little'),
            keys=[
                AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey="SysvarC1ock11111111111111111111111111111111", is_signer=False, is_writable=False),
            ]))
        result = send_transaction(client, trx, self.operator_acc)
        for log in result["result"]["meta"]["logMessages"]:
            log_words = log.split()
            if len(log_words) == 6 and log_words[2] == 'slot' and log_words[4] == 'blockhash':
                slot_hash[int(log_words[3])] = log_words[5]
        return slot_hash


    def get_blocks_from_solana(self):
        slot_hash = {}
        current_slot = client.get_slot()["result"]
        for slot in range(current_slot):
            hash_val = base58.b58decode(client.get_confirmed_block(slot)['result']['blockhash']).hex()
            slot_hash[int(slot)] = hash_val
        return slot_hash

    def test_01_block_hashes(self):
        print("test_01_block_hashes")
        solana_result = self.get_blocks_from_solana()
        nonevm_result = self.get_blocks_from_neonevm(100)
        for sol_slot, sol_hash in solana_result.items():
            if sol_slot in nonevm_result:
                self.assertEqual(sol_hash, nonevm_result[sol_slot])

if __name__ == '__main__':
    unittest.main()
