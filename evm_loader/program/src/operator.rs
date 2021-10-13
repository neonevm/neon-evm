//! OPERATOR LIST
#![allow(clippy::use_self)]

use solana_program::account_info::AccountInfo;
use crate::error::EvmLoaderError;

macros::pubkey_array!(
    AUTHORIZED_OPERATOR_LIST,
    [
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz1",
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz2",
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz3",
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz4",
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz5",
        "BMp6gEnveANdvSvspESJUrNczuHz1GF5UQKjVLCkAZih",
        "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRzZ",
    ]
);

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
