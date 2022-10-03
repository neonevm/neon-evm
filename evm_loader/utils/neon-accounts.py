#!/usr/bin/env python3
#
# File: neon-accounts.py
#
# Prints Ethereum accounts owned by the Neon EVM program.

import base64
import os

from solana.rpc.api import Client

SOLANA_URL = os.environ.get("SOLANA_URL", "http://solana:8899")
EVM_LOADER = os.environ.get("EVM_LOADER", "53DfF883gyixYNXnM7s5xhdeyV8mVk9T4i2hGV9vG9io")

NEON_ACCOUNT_MIN_SIZE = 71
TAG_ACCOUNT_V3 = 11


def process(account: object) -> int:
    result = 0

    data = account["data"]
    if data[1] != "base64":
        # print("Non-base64 data format")
        return result

    data = base64.b64decode(data[0])
    if len(data) < NEON_ACCOUNT_MIN_SIZE:
        # print("Non-Ethereum account: data size is too small", len(data))
        return result

    tag = data[0]
    address = "0x" + data[slice(1, 21)].hex()

    if tag == TAG_ACCOUNT_V3:
        print("Account V3:", address)
        result = 1

    return result


def main():
    client = Client(SOLANA_URL)
    response = client.get_program_accounts(EVM_LOADER, encoding="jsonParsed")

    count = 0
    for account in response["result"]:
        count += process(account["account"])

    print()
    print("Total Ethereum accounts V3:", count)


if __name__ == "__main__":
    main()
