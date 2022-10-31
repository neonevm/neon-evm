
#[must_use]
pub fn sha256(
    input: &[u8]
) -> Vec<u8> {
    use solana_program::hash::hash as sha256_digest;
    
    debug_print!("sha256");

    let hash = sha256_digest(input);

    hash.to_bytes().to_vec()
}