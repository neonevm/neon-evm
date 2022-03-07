#!/usr/bin/env python3
##
## File: neon-accounts.py
##
## Prints Ethereum accounts owned by the Neon EVM program.
## If executed with 'migrate' argument, performs the
## migration from V1-accounts to the current version.

import base64
import io
import os
import subprocess
import sys

from solana.rpc.api import Client

SOLANA_URL = os.environ.get("SOLANA_URL", "http://solana:8899")
EVM_LOADER = os.environ.get("EVM_LOADER", "53DfF883gyixYNXnM7s5xhdeyV8mVk9T4i2hGV9vG9io")

def do_migrate(address: str) -> None:
    cli = subprocess.Popen(["neon-cli-v2", "migrate-account", address,
                            "--url", SOLANA_URL, "--evm_loader", EVM_LOADER],
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    with io.TextIOWrapper(cli.stdout, encoding="utf-8") as out:
        for line in out:
            print(line.strip())

def process(account: object, command: str) -> (int, int):
    result = (0, 0)

    data = account["data"]
    if data[1] != "base64":
        # print("Non-base64 data format")
        return result

    data = base64.b64decode(data[0])
    if len(data) < 21:
        # print("Non-Ethereum account: data size is too small", len(data))
        return result

    tag = data[0]
    address = "0x" + data[slice(1, 21)].hex()

    if tag == 1:
        print("V1:", address)
        if command == "migrate":
            do_migrate(address)
        result = (1, 0)
    elif tag == 10:
        print("V2:", address)
        result = (0, 1)

    return result

def main():
    command = ""
    if len(sys.argv) > 1:
        command = sys.argv[1]

    client = Client(SOLANA_URL)
    response = client.get_program_accounts(EVM_LOADER, encoding="jsonParsed")

    count = (0, 0)
    for account in response["result"]:
        r = process(account["account"], command)
        count = (count[0] + r[0], count[1] + r[1])

    print()
    print("Total Ethereum accounts V1:", count[0])
    print("Total Ethereum accounts V2:", count[1])

if __name__ == "__main__":
    main()
