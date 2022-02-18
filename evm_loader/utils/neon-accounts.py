#!/usr/bin/env python3
##
## File: neon-accounts.py
##
## Prints accounts owned by the Neon EVM program.

import os
from solana.rpc.api import Client
from solana.publickey import PublicKey

SOLANA_URL = os.environ.get("SOLANA_URL", "http://solana:8899")
EVM_LOADER = os.environ.get("EVM_LOADER", "53DfF883gyixYNXnM7s5xhdeyV8mVk9T4i2hGV9vG9io")

client = Client(SOLANA_URL)

response = client.get_program_accounts(EVM_LOADER, encoding="base64")
#print(response)

for account in response["result"]:
    print(account)
