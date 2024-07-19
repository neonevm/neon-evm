#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unexpected response")]
    UnexpectedResponse,
    #[error("Program Account error")]
    ProgramAccountError,
    #[error("Rpc Client error {0:?}")]
    RpcClientError(#[from] solana_client::client_error::ClientError),
    #[error("IO error {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("Bincode error {0:?}")]
    BincodeError(#[from] bincode::Error),
    #[error("TryFromIntError error {0:?}")]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error("Transaction error {0:?}")]
    TransactionError(#[from] solana_sdk::transaction::TransactionError),
    #[error("Sanitize error {0:?}")]
    SanitizeError(#[from] solana_sdk::sanitize::SanitizeError),
    #[error("Instruction error {0:?}")]
    InstructionError(#[from] solana_sdk::instruction::InstructionError),
    #[error("Invalid ALT")]
    InvalidALT,
}
