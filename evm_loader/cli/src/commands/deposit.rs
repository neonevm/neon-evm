use evm::{H160};

use crate::{
    //errors::NeonCliError,
    Config,
    NeonCliResult,
};

#[allow(clippy::unnecessary_wraps)]
pub fn execute(
    config: &Config,
    amount: u64,
    ether_address: &H160,
) -> NeonCliResult {
    dbg!(config);
    dbg!(amount);
    dbg!(ether_address);
    Ok(())
}
