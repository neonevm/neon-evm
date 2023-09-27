use crate::{
    rpc::{self},
    Config, NeonError,
};
use solana_clap_utils::keypair::signer_from_path;
use solana_sdk::signature::Signer;

pub fn truncate_0x(in_str: &str) -> &str {
    if &in_str[..2] == "0x" {
        &in_str[2..]
    } else {
        in_str
    }
}

pub struct Context<'a> {
    pub rpc_client: &'a dyn rpc::Rpc,
    signer_config: &'a Config,
}

impl<'a> Context<'a> {
    pub fn signer(&self) -> Result<Box<dyn Signer>, NeonError> {
        build_signer(self.signer_config)
    }

    pub fn new(rpc_client: &'a dyn rpc::Rpc, signer_config: &'a Config) -> Context<'a> {
        Self {
            rpc_client,
            signer_config,
        }
    }
}

/// # Errors
pub fn build_signer(config: &Config) -> Result<Box<dyn Signer>, NeonError> {
    let mut wallet_manager = None;

    let signer = signer_from_path(
        &Default::default(),
        &config.keypair_path,
        "keypair",
        &mut wallet_manager,
    )
    .map_err(|_| NeonError::KeypairNotSpecified)?;

    Ok(signer)
}
