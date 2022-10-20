use serde::Deserialize;

#[derive(Deserialize)]
pub struct NetSpecificConfig {
    pub chain_id: u64,
    pub operators_whitelist: Vec<String>,
    pub token_mint: TokenMint,
    pub collateral_pool_base: CollateralPoolBase,
    pub account_whitelists: AccountWhitelists,
}

#[derive(Deserialize)]
pub struct TokenMint {
    pub neon_token_mint: String,
    pub decimals: u8,
}

#[derive(Deserialize)]
pub struct CollateralPoolBase {
    pub neon_pool_base: String,
    pub prefix: String,
    pub main_balance_seed: String,
    pub neon_pool_count: u32,
}

#[derive(Deserialize)]
pub struct AccountWhitelists {
    pub neon_permission_allowance_token: String,
    pub neon_permission_denial_token: String,
    pub neon_minimal_client_allowance_balance: String,
    pub neon_minimal_contract_allowance_balance: String,
}
