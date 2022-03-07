use crate::error::EvmLoaderError;
use solana_program::account_info::AccountInfo;
use solana_program::program_error::ProgramError;
use std::ops::Deref;

pub struct Operator<'a> {
    info: &'a AccountInfo<'a>,
}

impl<'a> Operator<'a> {
    pub fn from_account(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        let is_authorized = crate::config::AUTHORIZED_OPERATOR_LIST
            .iter()
            .any(|&item| item == *info.key);

        if !is_authorized {
            return Err!(EvmLoaderError::UnauthorizedOperator.into(); "Account {} - expected authorized operator", info.key);
        }

        unsafe { Self::from_account_not_whitelisted(info) }
    }

    /// # Safety
    /// Due to critical vulnerability, operator can destroy the world
    /// We trust whitelisted operators to not do this
    pub unsafe fn from_account_not_whitelisted(info: &'a AccountInfo<'a>) -> Result<Self, ProgramError> {
        if !solana_program::system_program::check_id(info.owner) {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected system owned", info.key);
        }

        if !info.is_signer {
            return Err!(ProgramError::InvalidArgument; "Account {} - expected signer", info.key);
        }

        Ok(Self { info })
    }
}

impl<'a> Deref for Operator<'a> {
    type Target = AccountInfo<'a>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}
