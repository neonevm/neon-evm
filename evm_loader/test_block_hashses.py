import unittest
import base58
from solana_utils import *

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
client = Client(solana_url)
CONTRACTS_DIR = os.environ.get("CONTRACTS_DIR", "evm_loader/")
evm_loader_id = os.environ.get("EVM_LOADER")
ETH_TOKEN_MINT_ID: PublicKey = PublicKey(os.environ.get("ETH_TOKEN_MINT"))


class DeployTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        print("\ntest_deploy.py setUpClass")

        operator_wallet = OperatorAccount(operator1_keypair_path())
        cls.operator_acc = operator_wallet.get_acc()


    def get_blocks_from_neonevm(self, count):
        trx = Transaction()
        trx.add(TransactionInstruction(program_id=evm_loader_id,
            data=bytes.fromhex('f0') + count.to_bytes(4, byteorder='little'),
            keys=[
                AccountMeta(pubkey="SysvarRecentB1ockHashes11111111111111111111", is_signer=False, is_writable=False),
                AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=False),
                AccountMeta(pubkey="SysvarC1ock11111111111111111111111111111111", is_signer=False, is_writable=False),
            ]))

        result = send_transaction(client, trx, self.operator_acc)
        meta_ixs = result['meta']['innerInstructions']

        for inner_ix in meta_ixs:
            for event in inner_ix['instructions']:
                log = base58.b58decode(event['data'])
                evm_ix = int(log[0])
                if evm_ix == 6:
                    self._decode_return(log)

    def _decode_return(self, log: bytes):
        slot = int.from_bytes(log[2:10], 'little')
        hash_val = log[10:].hex()
        print(f"slot: {slot}; hash:{hash_val}")

    def get_blocks_from_solana(self):
        current_slot = client.get_slot()["result"]
        for slot in range(current_slot):
            hash_val = base58.b58decode(client.get_confirmed_block(slot)['result']['blockhash']).hex()
            print(f"slot: {slot}; hash:{hash_val}")

    def test_01_block_hashes(self):
        print("test_01_block_hashes")
        self.get_blocks_from_solana()
        self.get_blocks_from_neonevm(1)
        self.get_blocks_from_neonevm(10)
        self.get_blocks_from_neonevm(100)
        self.get_blocks_from_neonevm(1000)

if __name__ == '__main__':
    unittest.main()
