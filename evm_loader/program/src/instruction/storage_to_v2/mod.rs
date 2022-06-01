use solana_program::pubkey::Pubkey;

pub mod write_value_to_distributed_storage;
pub mod convert_data_account_from_v1_to_v2;

// Migration operator public key: 6sXBjtBYNbUCKFq3CuAg7LHw9DJCvXujRUEFgK9TuzKx
const OPERATOR_PUBKEY: Pubkey = Pubkey::new_from_array([
    87, 59, 155, 5, 186, 78, 51, 40, 241, 253, 198, 247, 232, 155, 243, 95,
    148, 134, 196, 147, 252, 37, 178, 202, 185, 40, 42, 236, 179, 56, 216, 15,
]);
