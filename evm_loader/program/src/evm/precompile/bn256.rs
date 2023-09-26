use solana_program::alt_bn128::prelude::*;

/// Call inner `bn256Add`
#[must_use]
pub fn bn256_add(input: &[u8]) -> Vec<u8> {
    alt_bn128_addition(input).unwrap_or_else(|_| vec![])
}

/// Call inner `bn256ScalarMul`
#[must_use]
pub fn bn256_scalar_mul(input: &[u8]) -> Vec<u8> {
    alt_bn128_multiplication(input).unwrap_or_else(|_| vec![])
}

/// Call inner `bn256Pairing`
#[must_use]
pub fn bn256_pairing(input: &[u8]) -> Vec<u8> {
    alt_bn128_pairing(input).unwrap_or_else(|_| vec![])
}
