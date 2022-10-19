//! CONFIG MODULE
#![allow(clippy::useless_transmute)]

use cfg_if::cfg_if;
use const_format::formatcp;
use evm::U256;
use evm_loader_macro::{
    common_config_parser, declare_param_id, neon_elf_param, net_specific_config_parser,
    operators_whitelist,
};

use crate::account::ACCOUNT_SEED_VERSION;

cfg_if! {
    if #[cfg(feature = "mainnet")] {
        net_specific_config_parser!("config/mainnet.toml");
    } else if #[cfg(feature = "alpha")] {
        net_specific_config_parser!("config/alpha.toml");
    } else if #[cfg(feature = "testnet")] {
        net_specific_config_parser!("config/testnet.toml");
    } else if #[cfg(feature = "devnet")] {
        net_specific_config_parser!("config/devnet.toml");
    } else if #[cfg(feature = "govertest")] {
        net_specific_config_parser!("config/govertest.toml");
    } else {
        net_specific_config_parser!("config/default.toml");
    }
}

common_config_parser!("config/common.toml");

cfg_if! {
    if #[cfg(feature = "emergency")] {
        neon_elf_param!( NEON_STATUS_NAME, "EMERGENCY");
    } else {
        neon_elf_param!( NEON_STATUS_NAME, "WORK");
    }
}

neon_elf_param!(NEON_PKG_VERSION, env!("CARGO_PKG_VERSION"));
neon_elf_param!(NEON_REVISION, env!("NEON_REVISION"));
neon_elf_param!(NEON_SEED_VERSION, formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!(
    NEON_TOKEN_MINT_DECIMALS,
    formatcp!("{:?}", token_mint::DECIMALS)
);
neon_elf_param!(
    NEON_PAYMENT_TO_TREASURE,
    formatcp!("{:?}", PAYMENT_TO_TREASURE)
);
neon_elf_param!(
    NEON_PAYMENT_TO_DEPOSIT,
    formatcp!("{:?}", PAYMENT_TO_DEPOSIT)
);
neon_elf_param!(NEON_CHAIN_ID, formatcp!("{:?}", CHAIN_ID));
neon_elf_param!(
    NEON_POOL_COUNT,
    formatcp!("{:?}", collateral_pool_base::NEON_POOL_COUNT)
);
neon_elf_param!(NEON_HOLDER_MSG_SIZE, formatcp!("{:?}", HOLDER_MSG_SIZE));
neon_elf_param!(NEON_COMPUTE_UNITS, formatcp!("{:?}", COMPUTE_BUDGET_UNITS));
neon_elf_param!(
    NEON_HEAP_FRAME,
    formatcp!("{:?}", COMPUTE_BUDGET_HEAP_FRAME)
);
neon_elf_param!(
    NEON_ADDITIONAL_FEE,
    formatcp!("{:?}", REQUEST_UNITS_ADDITIONAL_FEE)
);
neon_elf_param!(
    NEON_GAS_LIMIT_MULTIPLIER_NO_CHAINID,
    formatcp!("{:?}", GAS_LIMIT_MULTIPLIER_NO_CHAINID)
);
neon_elf_param!(
    NEON_STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT,
    formatcp!("{:?}", STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT)
);
neon_elf_param!(NEON_EVM_STEPS_MIN, formatcp!("{:?}", EVM_STEPS_MIN));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
}
