from construct import Bytes, Int8ul, Struct, Int64ul, Int32ul

STORAGE_ACCOUNT_INFO_LAYOUT = Struct(
    "tag" / Int8ul,
    "caller" / Bytes(20),
    "nonce" / Int64ul,
    "gas_limit" / Bytes(32),
    "gas_price" / Bytes(32),
    "slot" / Int64ul,
    "operator" / Bytes(32),
    "account_list_len" / Int64ul,
    "executor_data_size" / Int64ul,
    "evm_data_size" / Int64ul,
    "gas_used_and_paid" / Bytes(32),
    "number_of_payments" / Int64ul,
    "signature" / Bytes(65),
)


FINALIZED_STORAGE_ACCOUNT_INFO_LAYOUT = Struct(
    "tag" / Int8ul,
    "sender" / Bytes(20),
    "signature" / Bytes(65),
)


ACCOUNT_INFO_LAYOUT = Struct(
    "type" / Int8ul,
    "ether" / Bytes(20),
    "nonce" / Int8ul,
    "trx_count" / Bytes(8),
    "balance" / Bytes(32),
    "is_rw_blocked" / Int8ul,
    "ro_blocked_cnt" / Int8ul,
    "generation" / Int32ul,
    "code_size" / Int32ul,
)

