from sha3 import keccak_256
import json
from web3.auto import w3
from eth_keys import keys
import struct


def unpack(data):
    ch = data[0]
    if ch <= 0x7F:
        return ch, data[1:]
    elif ch == 0x80:
        return None, data[1:]
    elif ch <= 0xB7:
        l = ch - 0x80
        return data[1:1 + l].tobytes(), data[1 + l:]
    elif ch <= 0xBF:
        lLen = ch - 0xB7
        l = int.from_bytes(data[1:1 + lLen], byteorder='big')
        return data[1 + lLen:1 + lLen + l].tobytes(), data[1 + lLen + l:]
    elif ch == 0xC0:
        return (), data[1:]
    elif ch <= 0xF7:
        l = ch - 0xC0
        lst = list()
        sub = data[1:1 + l]
        while len(sub):
            (item, sub) = unpack(sub)
            lst.append(item)
        return lst, data[1 + l:]
    else:
        lLen = ch - 0xF7
        l = int.from_bytes(data[1:1 + lLen], byteorder='big')
        lst = list()
        sub = data[1 + lLen:1 + lLen + l]
        while len(sub):
            (item, sub) = unpack(sub)
            lst.append(item)
        return lst, data[1 + lLen + l:]


def pack(data):
    if data is None:
        return (0x80).to_bytes(1, 'big')
    if isinstance(data, str):
        return pack(data.encode('utf8'))
    elif isinstance(data, bytes):
        if len(data) <= 55:
            return (len(data) + 0x80).to_bytes(1, 'big') + data
        else:
            l = len(data)
            lLen = (l.bit_length() + 7) // 8
            return (0xB7 + lLen).to_bytes(1, 'big') + l.to_bytes(lLen, 'big') + data
    elif isinstance(data, int):
        if data < 0x80:
            return data.to_bytes(1, 'big')
        else:
            l = (data.bit_length() + 7) // 8
            return (l + 0x80).to_bytes(1, 'big') + data.to_bytes(l, 'big')
        pass
    elif isinstance(data, list) or isinstance(data, tuple):
        if len(data) == 0:
            return (0xC0).to_bytes(1, 'big')
        else:
            res = bytearray()
            for d in data:
                res += pack(d)
            l = len(res)
            if l <= 55:
                return (l + 0xC0).to_bytes(1, 'big') + res
            else:
                lLen = (l.bit_length() + 7) // 8
                return (lLen + 0xF7).to_bytes(1, 'big') + l.to_bytes(lLen, 'big') + res
    else:
        raise Exception("Unknown type {} of data".format(str(type(data))))


def get_int(a):
    if isinstance(a, int):
        return a
    if isinstance(a, bytes):
        return int.from_bytes(a, 'big')
    if a is None:
        return a
    raise Exception("Invalid conversion from {} to int".format(a))


class Trx:
    def __init__(self):
        self.nonce = None
        self.gasPrice = None
        self.gasLimit = None
        self.toAddress = None
        self.value = None
        self.callData = None
        self.v = None
        self.r = None
        self.s = None

    @classmethod
    def from_string(cls, s):
        t = Trx()
        (unpacked, data) = unpack(memoryview(s))
        (nonce, gasPrice, gasLimit, toAddress, value, callData, v, r, s) = unpacked
        t.nonce = get_int(nonce)
        t.gasPrice = get_int(gasPrice)
        t.gasLimit = get_int(gasLimit)
        t.toAddress = toAddress
        t.value = get_int(value)
        t.callData = callData
        t.v = get_int(v)
        t.r = get_int(r)
        t.s = get_int(s)
        return t

    def chain_id(self):
        # chainid*2 + 35  xxxxx0 + 100011   xxxx0 + 100010 +1
        # chainid*2 + 36  xxxxx0 + 100100   xxxx0 + 100011 +1
        return (self.v - 1) // 2 - 17

    def __str__(self):
        return pack((
            self.nonce,
            self.gasPrice,
            self.gasLimit,
            self.toAddress,
            self.value,
            self.callData,
            self.v,
            self.r.to_bytes(32, 'big') if self.r else None,
            self.s.to_bytes(32, 'big') if self.s else None)
        ).hex()

    def get_msg(self, chain_id=None):
        return pack((
            self.nonce,
            self.gasPrice,
            self.gasLimit,
            self.toAddress,
            self.value,
            self.callData,
            chain_id or self.chain_id(), None, None))

    def hash(self, chain_id=None):
        trx = pack((
            self.nonce,
            self.gasPrice,
            self.gasLimit,
            self.toAddress,
            self.value,
            self.callData,
            chain_id or self.chain_id(), None, None))
        return keccak_256(trx).digest()

    def sender(self):
        msg_hash = self.hash()
        sig = keys.Signature(vrs=[1 if self.v % 2 == 0 else 0, self.r, self.s])
        pub = sig.recover_public_key_from_msg_hash(msg_hash)
        return pub.to_canonical_address().hex()


class JsonEncoder(json.JSONEncoder):
    def default(self, obj):
        if isinstance(obj, bytes):
            return obj.hex()
        return json.JSONEncoder.default(obj)


def make_instruction_data_from_tx(instruction, private_key=None):
    if isinstance(instruction, dict):
        if instruction['chainId'] is None:
            raise Exception("chainId value is needed in input dict")
        if private_key is None:
            raise Exception(
                "Needed private key for transaction creation from fields")

        signed_tx = w3.eth.account.sign_transaction(instruction, private_key)
        # print(signed_tx.rawTransaction.hex())
        _trx = Trx.from_string(signed_tx.rawTransaction)
        # print(json.dumps(_trx.__dict__, cls=JsonEncoder, indent=3))

        raw_msg = _trx.get_msg(instruction['chainId'])
        sig = keys.Signature(vrs=[1 if _trx.v % 2 == 0 else 0, _trx.r, _trx.s])
        pub = sig.recover_public_key_from_msg_hash(_trx.hash())

        # print(pub.to_hex())

        return pub.to_canonical_address(), sig.to_bytes(), raw_msg
    elif isinstance(instruction, str):
        if instruction[:2] == "0x":
            instruction = instruction[2:]

        _trx = Trx.from_string(bytearray.fromhex(instruction))
        # print(json.dumps(_trx.__dict__, cls=JsonEncoder, indent=3))

        raw_msg = _trx.get_msg()
        sig = keys.Signature(vrs=[1 if _trx.v % 2 == 0 else 0, _trx.r, _trx.s])
        pub = sig.recover_public_key_from_msg_hash(_trx.hash())

        data = pub.to_canonical_address()
        data += sig.to_bytes()
        data += raw_msg

        return pub.to_canonical_address(), sig.to_bytes(), raw_msg
    else:
        raise Exception("function gets ")


def make_keccak_instruction_data(check_instruction_index, msg_len, data_start):
    if 255 < check_instruction_index < 0:
        raise Exception(
            "Invalid index for instruction - {}".format(check_instruction_index))

    check_count = 1
    eth_address_size = 20
    signature_size = 65
    eth_address_offset = data_start
    signature_offset = eth_address_offset + eth_address_size
    message_data_offset = signature_offset + signature_size

    data = struct.pack("B", check_count)
    data += struct.pack("<H", signature_offset)
    data += struct.pack("B", check_instruction_index)
    data += struct.pack("<H", eth_address_offset)
    data += struct.pack("B", check_instruction_index)
    data += struct.pack("<H", message_data_offset)
    data += struct.pack("<H", msg_len)
    data += struct.pack("B", check_instruction_index)

    return data
