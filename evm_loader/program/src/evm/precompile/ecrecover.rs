use arrayref::{array_ref, array_refs};
use ethnum::U256;
use solana_program::keccak;
use solana_program::secp256k1_recover::secp256k1_recover;

#[allow(clippy::manual_let_else)]
#[must_use]
pub fn ecrecover(input: &[u8]) -> Vec<u8> {
    debug_print!("ecrecover");

    let input = if input.len() >= 128 {
        input[..128].to_vec()
    } else {
        let mut buffer = vec![0_u8; 128];
        buffer[..input.len()].copy_from_slice(input);
        buffer
    };

    let data = array_ref![input, 0, 128];
    let (msg, v, sig) = array_refs![data, 32, 32, 64];

    let v = U256::from_be_bytes(*v);
    if !(27..=30).contains(&v) {
        return vec![];
    }

    let recovery_id = v.as_u8() - 27;

    let Ok(public_key) = secp256k1_recover(&msg[..], recovery_id, &sig[..]) else {
        return vec![];
    };

    let mut address = keccak::hash(&public_key.to_bytes()).to_bytes();
    address[0..12].fill(0);

    debug_print!("{}", hex::encode(address));
    address.to_vec()
}
