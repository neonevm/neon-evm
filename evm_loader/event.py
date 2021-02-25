from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *


solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
# evm_loader = os.environ.get("EVM_LOADER")
evm_loader = "HiXxTHaNAWRoTonrEee8HUaoHzb8KZKLAb4QQJHDKY97"
owner_contract = os.environ.get("CONTRACT")

def wallet_path():
    cmd = 'solana --url {} config get'.format(solana_url)
    try:
        res =  subprocess.check_output(cmd, shell=True, universal_newlines=True)
        res = res.splitlines()[-1]
        substr = "Keypair Path: "
        if not res.startswith(substr):
            raise Exception("cannot get keypair path")
        path = res[len(substr):]
        return path.strip()
    except subprocess.CalledProcessError as err:
        import sys

        print("ERR: solana error {}".format(err))
        raise

class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        wallet = RandomAccaunt(wallet_path())
        cls.loader = EvmLoader(solana_url, wallet, evm_loader)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = solana2ether(cls.acc.public_key())
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

    def call(self, contract, data, raw_result=False, run_tx=True):
        accounts = [
            AccountMeta(pubkey=contract, is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
            AccountMeta(pubkey=self.acc.public_key(), is_signer=True, is_writable=False),
            AccountMeta(pubkey=self.loader.loader_id, is_signer=False, is_writable=False),
            AccountMeta(pubkey=PublicKey("SysvarC1ock11111111111111111111111111111111"), is_signer=False,
                        is_writable=False),
        ]

        instr = TransactionInstruction(program_id=self.loader.loader_id, data=data, keys=accounts)
        if not run_tx:
            return instr
        trx = Transaction().add(instr)
        result = \
            http_client.send_transaction(trx, self.acc,
                                         opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))[
                "result"]
        if raw_result:
            return result
        messages = result["meta"]["logMessages"]
        res = messages[messages.index("Program log: succeed") + 1]
        if not res.startswith("Program log: "):
            raise Exception("Invalid program logs: no result")
        else:
            return bytearray.fromhex(res[13:])

    def test_evmloader_returns_events(self):
        # reId = self.loader.deployChecked("event.bin", bytes(self.acc.public_key()))
        reId = "42NvfP6v8u3JvARwjEMCwmSauF8c7u7edvvULSW59MkL"
        print("ReturnsEvents program:", reId)

        # Call addNoReturn for returnsevents
        data = (bytes.fromhex("03c2eb5db1") +  # 03 means call, next part means addNoReturn(uint8,uint8)
                bytes.fromhex("%064x" % 0x1) +
                bytes.fromhex("%064x" % 0x2)
                )
        print('addNoReturn arguments:', data.hex())
        result = self.call(
            contract=reId,
            data=data,
            raw_result=True)
        print('addNoReturn result:', result)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data, b'\x06')  # 6 means OnReturn, and no value next - empty
        print('')

        # Call addReturn for returnsevents
        data = (bytes.fromhex("03c14f01d7") +  # 03 means call, next part means addReturn(uint8,uint8)
                bytes.fromhex("%064x" % 0x1) +
                bytes.fromhex("%064x" % 0x2)
                )
        print('addReturn arguments:', data.hex())
        result = self.call(
            contract=reId,
            data=data,
            raw_result=True)
        print('addReturn result:', result)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x06')
        self.assertEqual(data[1:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        print('')

        # Call addReturnEvent for returnsevents
        data = (bytes.fromhex("030049a148") +  # 03 means call, next part means addReturnEvent(uint8,uint8)
                bytes.fromhex("%064x" % 0x1) +
                bytes.fromhex("%064x" % 0x2)
                )
        print('addReturnEvent arguments:', data.hex())
        result = self.call(
            contract=reId,
            data=data,
            raw_result=True)
        print('addReturnEvent result:', result)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        self.assertEqual(data[-32:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x06')
        self.assertEqual(data[1:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        print('')

        # Call addReturnEventTwice for returnsevents
        data = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
                bytes.fromhex("%064x" % 0x1) +
                bytes.fromhex("%064x" % 0x2)
                )
        print('addReturnEventTwice arguments:', data.hex())
        result = self.call(
            contract=reId,
            data=data,
            raw_result=True)
        print('addReturnEventTwice result:', result)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data[:1], b'\x07')
        self.assertEqual(data[-32:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        self.assertEqual(data[:1], b'\x07')
        self.assertEqual(data[-32:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x05')
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][2]['data'])
        self.assertEqual(data[:1], b'\x06')
        self.assertEqual(data[1:],
                         b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x05')
        print('')

        # Call addReturnEventTwice 2 times in same trx
        data1 = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
                 bytes.fromhex("%064x" % 0x1) +
                 bytes.fromhex("%064x" % 0x2)
                 )
        data2 = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
                 bytes.fromhex("%064x" % 0x3) +
                 bytes.fromhex("%064x" % 0x4)
                 )
        print('addReturnEventTwice*2 arguments:', data.hex())
        trx = Transaction().add(self.call(
            contract=reId,
            data=data1,
            run_tx=False)).add(self.call( contract=reId, data=data2, run_tx=False))
        result = \
        http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))[
            "result"]
        print('addReturnEventTwice*2 result:', result)

if __name__ == '__main__':
    unittest.main()