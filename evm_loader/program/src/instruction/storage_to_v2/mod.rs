use solana_program::pubkey::Pubkey;

pub mod write_value_to_distributed_storage;
pub mod convert_data_account_from_v1_to_v2;

const OPERATOR_PUBKEY: Pubkey = Pubkey::new_from_array([0; 32]);
