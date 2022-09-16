from construct import Bytes, Int8ul, Struct, Int64ul, Int32ul

STORAGE_ACCOUNT_INFO_LAYOUT = Struct(
    "tag" / Int8ul,
    "owner" / Bytes(32),
    "hash" / Bytes(32),
    "caller" / Bytes(20),
    "gas_limit" / Bytes(32),
    "gas_price" / Bytes(32),
    "gas_used" / Bytes(32),
    "operator" / Bytes(32),
    "slot" / Int64ul,
    "account_list_len" / Int64ul,
)


FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT = Struct(
    "tag" / Int8ul,
    "owner" / Bytes(32),
    "hash" / Bytes(32),
)


ACCOUNT_INFO_LAYOUT = Struct(
    "type" / Int8ul,
    "ether" / Bytes(20),
    "nonce" / Int8ul,
    "trx_count" / Bytes(8),
    "balance" / Bytes(32),
    "generation" / Int32ul,
    "code_size" / Int32ul,
    "is_rw_blocked" / Int8ul,
)


CREATE_ACCOUNT_LAYOUT = Struct(
    "ether" / Bytes(20),
)
