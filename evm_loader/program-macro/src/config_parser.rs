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

#[derive(Deserialize)]
pub struct CommonConfig {
    pub payment_to_treasure: u64,
    pub payment_to_deposit: u64,
    pub operator_priority_slots: u64,
    pub holder_msg_size: u64,
    pub compute_budget_units: u32,
    pub compute_budget_heap_frame: u32,
    pub request_units_additional_fee: u64,
    pub gas_limit_multiplier_no_chainid: u32,
    pub storage_entries_in_contract_account: u32,
    pub evm_steps_min: u64,
    pub evm_steps_last_iteration_max: u64,
}
