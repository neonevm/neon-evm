//! CONFIG MODULE
#![allow(clippy::useless_transmute)]

use cfg_if::cfg_if;
use evm_loader_macro::{common_config_parser, neon_elf_param, net_specific_config_parser};

cfg_if! {
    if #[cfg(feature = "mainnet")] {
        net_specific_config_parser!("config/mainnet.toml");
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

cfg_if! {
    if #[cfg(feature = "emergency")] {
        neon_elf_param!(NEON_STATUS_NAME, "EMERGENCY");
    } else {
        neon_elf_param!(NEON_STATUS_NAME, "WORK");
    }
}

common_config_parser!("config/common.toml");

neon_elf_param!(NEON_PKG_VERSION, env!("CARGO_PKG_VERSION"));
neon_elf_param!(NEON_REVISION, env!("NEON_REVISION"));
