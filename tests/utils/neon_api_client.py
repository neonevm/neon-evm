import requests

from .constants import CHAIN_ID, NEON_TOKEN_MINT_ID


class NeonApiClient:
    def __init__(self, url):
        self.url = url
        self.headers = {"Content-Type": "application/json"}

    def emulate(self, sender, contract, data=bytes(), chain_id=CHAIN_ID, value='0x0', max_steps_to_execute=500000):
        body = {
            "step_limit": max_steps_to_execute,
            "tx": {
                "from": sender,
                "to": contract,
                "data": data.hex(),
                "chain_id": chain_id,
                "value": value
            },
            "accounts": []
        }

        resp = requests.post(url=f"{self.url}/emulate", json=body, headers=self.headers)
        print(resp.text)
        return resp.json()

    def get_storage_at(self, contract_id, index="0x0"):
        body = {
            "contract": contract_id,
            "index": index
        }
        return requests.post(url=f"{self.url}/storage", json=body, headers=self.headers).json()

    def get_ether_account_data(self, ether, chain_id = CHAIN_ID):
        body = {
            "account": [
                { "address": ether, "chain_id": chain_id }
            ]
        }
        return requests.post(url=f"{self.url}/balance", json=body, headers=self.headers).json()