//! OPERATOR LIST
#![allow(clippy::use_self)]
#![allow(clippy::missing_errors_doc)]

use solana_program::account_info::AccountInfo;
use crate::error::EvmLoaderError;
use crate::config::AUTHORIZED_OPERATOR_LIST;

/// Authorized operator check
pub fn authorized_operator_check(account_info: &AccountInfo) -> Result<(),EvmLoaderError> {
    if account_info.is_signer {
        AUTHORIZED_OPERATOR_LIST
            .iter()
            .find(|&&item| item == *account_info.key )
            .map(|_|())
            .ok_or(EvmLoaderError::UnauthorizedOperator)
    } else {
        Err(EvmLoaderError::UnauthorizedOperator)
    }
}
