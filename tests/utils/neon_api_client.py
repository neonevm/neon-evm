import os

import requests


class NeonApiClient:
    def __init__(self, url):
        self.url = url
        self.token_mint = os.environ.get('NEON_TOKEN_MINT')
        self.chain_id = os.environ.get('NEON_CHAIN_ID')
        self.headers = {"Content-Type": "application/json"}

    def emulate(self, sender, contract, gas_limit=None, data=None, token_mint=None, chain_id=None,
                cached_accounts=None, solana_accounts=None, max_steps_to_execute=500000,
                value='0x0'):
        token_mint = self.token_mint if token_mint is None else token_mint
        chain_id = int(self.chain_id) if chain_id is None else chain_id

        body = {"token_mint": token_mint, "chain_id": chain_id,
                "max_steps_to_execute": max_steps_to_execute, "cached_accounts": cached_accounts,
                "solana_accounts": solana_accounts,
                "sender": sender,
                "contract": contract, "value": value, "gas_limit": gas_limit}
        if contract:
            body["contract"] = contract
        if data:
            body['data'] = list(data)
        resp = requests.post(url=f"{self.url}/emulate", json=body, headers=self.headers)
        return resp.json()

    def get_storage_at(self, contract_id, index="0x0"):
        return requests.get(f"{self.url}/get-storage-at?contract_id={contract_id}&index={index}").json()

    def get_ether_account_data(self, ether):
        return requests.get(f"{self.url}/get-ether-account-data?ether={ether}").json()
