// Should be implemented via Solana syscall
// use solana_program::alt_bn128::prelude::*;

/// Call inner `bn256Add`
#[must_use]
pub fn bn256_add(_input: &[u8]) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()
    //alt_bn128_addition(input).unwrap_or_else(|_| vec![])
}

/// Call inner `bn256ScalarMul`
#[must_use]
pub fn bn256_scalar_mul(_input: &[u8]) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()
    //alt_bn128_multiplication(input).unwrap_or_else(|_| vec![])
}

/// Call inner `bn256Pairing`
#[must_use]
pub fn bn256_pairing(_input: &[u8]) -> Vec<u8> {
    // Should be implemented via Solana syscall
    Vec::new()
    //alt_bn128_pairing(input).unwrap_or_else(|_| vec![])
}
