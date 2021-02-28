from solana.transaction import AccountMeta, TransactionInstruction, Transaction
from solana.rpc.types import TxOpts
import unittest
from base58 import b58decode
from solana_utils import *
from eth_tx_utils import make_keccak_instruction_data, make_instruction_data_from_tx

solana_url = os.environ.get("SOLANA_URL", "http://localhost:8899")
http_client = Client(solana_url)
# evm_loader = os.environ.get("EVM_LOADER")
evm_loader_id = "3hW56nLdw595VxjHJrYZcBsVe2fduEtoedFixdPbdYUH"
owner_contract = os.environ.get("CONTRACT")
sysinstruct = "Sysvar1nstructions1111111111111111111111111"
keccakprog = "KeccakSecp256k11111111111111111111111111111"
sysvarclock = "SysvarC1ock11111111111111111111111111111111"


class EventTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        wallet = WalletAccount(wallet_path())
        cls.loader = EvmLoader(solana_url, wallet, evm_loader_id)
        cls.acc = wallet.get_acc()

        # Create ethereum account for user account
        cls.caller_ether = eth_keys.PrivateKey(cls.acc.secret_key()).public_key.to_canonical_address()
        (cls.caller, cls.caller_nonce) = cls.loader.ether2program(cls.caller_ether)

        if getBalance(cls.caller) == 0:
            print("Create caller account...")
            _ = cls.loader.createEtherAccount(cls.caller_ether)
            print("Done\n")

        print('Account:', cls.acc.public_key(), bytes(cls.acc.public_key()).hex())
        print("Caller:", cls.caller_ether.hex(), cls.caller_nonce, "->", cls.caller,
              "({})".format(bytes(PublicKey(cls.caller)).hex()))

        cls.reId = cls.loader.deployChecked("event.bin", solana2ether(cls.acc.public_key()))


    def call_signed(self, input, raw_result):
        tx = {
            'to': solana2ether(self.reId),
            'value': 1,
            'gas': 1,
            'gasPrice': 1,
            'nonce': getTransactionCount(http_client, self.caller),
            'data': input,
            'chainId': 1
        }
        (from_addr, sign, msg) = make_instruction_data_from_tx(tx, self.acc.secret_key())
        assert (from_addr == self.caller_ether)
        keccak_instruction = make_keccak_instruction_data(1, len(msg))

        evm_instruction = from_addr + sign + msg
        trx = Transaction().add(
            TransactionInstruction(program_id=keccakprog, data=keccak_instruction, keys=[
                AccountMeta(pubkey=PublicKey(keccakprog), is_signer=False, is_writable=False), ])).add(
            TransactionInstruction(program_id=evm_loader_id,
                                   data=bytearray.fromhex("05") + evm_instruction,
                                   keys=[
                                       AccountMeta(pubkey=self.reId, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=self.caller, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysinstruct), is_signer=False, is_writable=False),
                                       AccountMeta(pubkey=evm_loader_id, is_signer=False, is_writable=True),
                                       AccountMeta(pubkey=PublicKey(sysvarclock), is_signer=False, is_writable=False),
                                   ]))

        result = http_client.send_transaction(trx, self.acc,
                                     opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))["result"]
        if raw_result:
            return result
        messages = result["meta"]["logMessages"]
        res = messages[messages.index("Program log: succeed") + 1]
        if not res.startswith("Program log: "):
            raise Exception("Invalid program logs: no result")
        else:
            return bytearray.fromhex(res[13:])

    def test_evmloader_returns_events(self):
        print("ReturnsEvents program:", self.reId)

        # Call addNoReturn for returnsevents
        data = (bytes.fromhex("c2eb5db1") +
                bytes.fromhex("%064x" % 0x1) +
                bytes.fromhex("%064x" % 0x2)
                )
        print('addNoReturn arguments:', data.hex())
        result = self.call_signed(
            input=data,
            raw_result=True)
        print('addNoReturn result:', result)
        self.assertEqual(result['meta']['err'], None)
        self.assertEqual(len(result['meta']['innerInstructions']), 1)
        self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        self.assertEqual(data, b'\x06')  # 6 means OnReturn, and no value next - empty
        print('')

        # # Call addReturn for returnsevents
        # data = (bytes.fromhex("03c14f01d7") +  # 03 means call, next part means addReturn(uint8,uint8)
        #         bytes.fromhex("%064x" % 0x1) +
        #         bytes.fromhex("%064x" % 0x2)
        #         )
        # print('addReturn arguments:', data.hex())
        # result = self.call(
        #     contract=self.reId,
        #     data=data,
        #     raw_result=True)
        # print('addReturn result:', result)
        # self.assertEqual(result['meta']['err'], None)
        # self.assertEqual(len(result['meta']['innerInstructions']), 1)
        # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 1)
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        # self.assertEqual(data[:1], b'\x06')
        # self.assertEqual(data[1:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        # print('')
        #
        # # Call addReturnEvent for returnsevents
        # data = (bytes.fromhex("030049a148") +  # 03 means call, next part means addReturnEvent(uint8,uint8)
        #         bytes.fromhex("%064x" % 0x1) +
        #         bytes.fromhex("%064x" % 0x2)
        #         )
        # print('addReturnEvent arguments:', data.hex())
        # result = self.call(
        #     contract=self.reId,
        #     data=data,
        #     raw_result=True)
        # print('addReturnEvent result:', result)
        # self.assertEqual(result['meta']['err'], None)
        # self.assertEqual(len(result['meta']['innerInstructions']), 1)
        # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 2)
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        # self.assertEqual(data[:1], b'\x07')  # 7 means OnEvent
        # self.assertEqual(data[-32:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        # self.assertEqual(data[:1], b'\x06')
        # self.assertEqual(data[1:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        # print('')
        #
        # # Call addReturnEventTwice for returnsevents
        # data = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
        #         bytes.fromhex("%064x" % 0x1) +
        #         bytes.fromhex("%064x" % 0x2)
        #         )
        # print('addReturnEventTwice arguments:', data.hex())
        # result = self.call(
        #     contract=self.reId,
        #     data=data,
        #     raw_result=True)
        # print('addReturnEventTwice result:', result)
        # self.assertEqual(result['meta']['err'], None)
        # self.assertEqual(len(result['meta']['innerInstructions']), 1)
        # self.assertEqual(len(result['meta']['innerInstructions'][0]['instructions']), 3)
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][0]['data'])
        # self.assertEqual(data[:1], b'\x07')
        # self.assertEqual(data[-32:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x03')
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][1]['data'])
        # self.assertEqual(data[:1], b'\x07')
        # self.assertEqual(data[-32:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x05')
        # data = b58decode(result['meta']['innerInstructions'][0]['instructions'][2]['data'])
        # self.assertEqual(data[:1], b'\x06')
        # self.assertEqual(data[1:],
        #                  b'\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x05')
        # print('')
        #
        # # Call addReturnEventTwice 2 times in same trx
        # data1 = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
        #          bytes.fromhex("%064x" % 0x1) +
        #          bytes.fromhex("%064x" % 0x2)
        #          )
        # data2 = (bytes.fromhex("036268c754") +  # 03 means call, next part means addReturnEventTwice(uint8,uint8)
        #          bytes.fromhex("%064x" % 0x3) +
        #          bytes.fromhex("%064x" % 0x4)
        #          )
        # print('addReturnEventTwice*2 arguments:', data.hex())
        # trx = Transaction().add(self.call(
        #     contract=self.reId,
        #     data=data1,
        #     run_tx=False)).add(self.call( contract=reId, data=data2, run_tx=False))
        # result = \
        # http_client.send_transaction(trx, self.acc, opts=TxOpts(skip_confirmation=False, preflight_commitment="root"))[
        #     "result"]
        # print('addReturnEventTwice*2 result:', result)

if __name__ == '__main__':
    unittest.main()