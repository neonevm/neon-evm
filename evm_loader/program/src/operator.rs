//! OPERATOR LIST
use solana_program::{
    account_info::AccountInfo,
};
use crate::error::EvmLoaderError;

const AUTHORIZED_OPERATOR_LIST: [[u8; 32]; 7] = [
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz1"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 50],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz2"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 51],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz3"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 52],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz4"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 53],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz5"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 54],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRz6"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 55],
    //  "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRzZ"
    [170, 167, 207, 51, 83, 78, 243, 181, 91, 59, 55, 179, 27, 200, 225, 252, 49, 156, 84, 238, 23, 89, 149, 83, 70, 65, 24, 203, 57, 60, 52, 82],
];

/// Authorized operator check
pub fn authorized_operator_check(account_info: &AccountInfo) -> Result<(),EvmLoaderError> {
    if account_info.is_signer {
        AUTHORIZED_OPERATOR_LIST
            .iter()
            .find(|&&item| item == account_info.key.to_bytes() )
            .map(|_|())
            .ok_or(EvmLoaderError::UnauthorizedOperator)
    } else {
        Err(EvmLoaderError::UnauthorizedOperator)
    }
}