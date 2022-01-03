//! Faucet manual module.

const MANUAL: &str = r##"
# API Endpoints

|:-:|:-:|-
|**Endpoint**|**Workload**|**Description**|
|:-|:-:|-
| request_neon_in_galans | JSON | Requests NEON tokens, amount in galans (fractions)
| request_neon | JSON | Requests NEON tokens
| request_erc20 | JSON | Requests ERC20 tokens
| request_stop | | Initiates graceful shutdown
|-

Workload JSON schema:
```
{
"type": "object",
    "properties": {
        "wallet": {
            "type": "string",
            "description": "Address of an Ethereum account"
        },
        "amount": {
            "type": "integer",
            "description": "Amount of tokens to receive",
        }
    }
}
```

Workload JSON example:
```
{ "wallet": "0x4570e07200b6332989Dc04fA2a671b839D26eF0E", "amount": 1 }
```

# Configuration

Example of the configuration file:
```
[rpc]
bind = "0.0.0.0"
port = 3333
allowed_origins = ["http://localhost"]

[web3]
enable = true
rpc_url = "http://localhost:9090/solana"
private_key = "0x0000000000000000000000000000000000000000000000000000000000000Ace"
tokens = ["0x00000000000000000000000000000000CafeBabe",
          "0x00000000000000000000000000000000DeadBeef"]
max_amount = 1000

[solana]
enable = true
url = "http://localhost:8899"
evm_loader = "EvmLoaderId11111111111111111111111111111111"
token_mint = "TokenMintId11111111111111111111111111111111"
token_mint_decimals = 9
operator_keyfile = "operator_id.json"
max_amount = 10
```

The configuration file is optional and, if present, can be incomplete.

# Environment Variables

Environment variables, if present, override portions of the configuration.

|:-:|:-:|-
|**Name**|**Overrides**|**Value Example**|
|:-|:-|-
| FAUCET_RPC_BIND | rpc.bind | `"0.0.0.0"` 
| FAUCET_RPC_PORT | rpc.port | `3333`
| FAUCET_RPC_ALLOWED_ORIGINS | rpc.allowed_origins | `["http://localhost"]`
| FAUCET_WEB3_ENABLE | web3.enable | `true`
| WEB3_RPC_URL | web3.rpc_url | `"http://localhost:9090/solana"`
| WEB3_PRIVATE_KEY | web3.private_key | `"0x00A"`
| NEON_ERC20_TOKENS | web3.tokens | `["0x00B", "0x00C"]`
| NEON_ERC20_MAX_AMOUNT | web3.max_amount | `1000`
| FAUCET_SOLANA_ENABLE | solana.enable | `true`
| SOLANA_URL | solana.url | `"http://localhost:8899"`
| EVM_LOADER | solana.evm_loader | `"EvmLoaderId11111111111111111111111111111111"`
| NEON_TOKEN_MINT | solana.token_mint | `"TokenMintId11111111111111111111111111111111"`
| NEON_TOKEN_MINT_DECIMALS | solana.token_mint_decimals | `9`
| NEON_OPERATOR_KEYFILE | solana.operator_keyfile | `"operator_id.json"`
| NEON_ETH_MAX_AMOUNT | solana.max_amount | `10`
|-
"##;

//use crossterm::style::Color::Yellow;
use minimad::Alignment;
use termimad::MadSkin;

pub fn show() {
    let mut skin = MadSkin::default();
    skin.headers[0].align = Alignment::Left;
    skin.print_text(MANUAL);
}
