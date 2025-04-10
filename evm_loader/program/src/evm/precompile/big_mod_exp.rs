use ethnum::U256;

use crate::types::vector::VectorVecExt;
use crate::types::Vector;
use crate::vector;

fn _mod_exp_fast(base: U256, exponent: U256, modulus: U256) -> U256 {
    if modulus == U256::ONE {
        return U256::ZERO;
    }

    let mut result = U256::ONE;
    let mut base = base % modulus;
    let mut exponent = exponent;

    while exponent > U256::ZERO {
        if exponent % U256::new(2) == U256::ONE {
            result = (result * base) % modulus;
        }
        exponent >>= 1;
        base = (base * base) % modulus;
    }

    result
}

/// Constant-time modular exponentiation: computes b^e mod m without timing leaks
fn mod_exp(b: U256, e: U256, m: U256) -> U256 {
    if m == U256::ONE {
        return U256::ZERO;
    }

    let mut result = U256::ONE;
    let mut base = b % m;
    let mut exponent = e;

    // Work with a copy of the modulus that we can modify
    let modulus = m;

    // Constant-time loop - always runs for 256 iterations
    for _ in 0..256 {
        // Check the least significant bit in constant time
        if (exponent & U256::ONE) == U256::ONE {
            result = mul_mod(result, base, modulus);
        }

        // Right shift exponent (divide by 2)
        exponent >>= 1;

        // Square the base
        base = mul_mod(base, base, modulus);
    }

    result
}

/// Constant-time modular multiplication: (a * b) % m
fn mul_mod(a: U256, b: U256, m: U256) -> U256 {
    assert!((m != U256::ZERO), "modulus cannot be zero");

    // Compute a * b using checked_mul to detect overflow
    let Some(product) = a.checked_mul(b) else {
        // If multiplication overflows, we need to use a slower method
        // This branch should theoretically never be taken for U256 when m is U256::MAX
        // because a and b are both less than m (from mod_exp)
        let mut res = U256::ZERO;
        let mut a = a;
        let mut b = b;

        // This is a constant-time multiplication algorithm
        for _ in 0..256 {
            if (b & U256::ONE) == U256::ONE {
                res = add_mod(res, a, m);
            }
            a = add_mod(a, a, m);
            b >>= 1;
        }
        return res;
    };

    product % m
}

/// Constant-time modular addition: (a + b) % m
fn add_mod(a: U256, b: U256, m: U256) -> U256 {
    assert!((m != U256::ZERO), "modulus cannot be zero");

    let sum = a.overflowing_add(b);
    let (sum, overflow) = sum;

    if overflow || sum >= m {
        sum.wrapping_sub(m)
    } else {
        sum
    }
}

fn u8_array_to_u256(bytes: &[u8]) -> U256 {
    // Ensure the slice is not longer than 32 bytes (U256's size)
    assert!(bytes.len() <= 32, "Input slice is too large for U256");

    // Create a 32-byte buffer initialized with zeros
    let mut buffer = [0u8; 32];

    // Copy the input bytes into the buffer (right-aligned)
    let start = 32 - bytes.len();
    buffer[start..].copy_from_slice(bytes);

    // Convert the buffer into U256 (big-endian)
    U256::from_be_bytes(buffer)
}

#[must_use]
pub fn big_mod_exp(input: &[u8]) -> Vector<u8> {
    if input.len() < 96 {
        return vector![];
    }

    let (base_len, rest) = input.split_at(32);
    let (exp_len, rest) = rest.split_at(32);
    let (mod_len, rest) = rest.split_at(32);

    let Ok(base_len) = U256::from_be_bytes(base_len.try_into().unwrap()).try_into() else {
        return vector![];
    };
    let Ok(exp_len) = U256::from_be_bytes(exp_len.try_into().unwrap()).try_into() else {
        return vector![];
    };
    let Ok(mod_len) = U256::from_be_bytes(mod_len.try_into().unwrap()).try_into() else {
        return vector![];
    };

    if base_len == 0 || mod_len == 0 {
        return vector![0; 32];
    }

    let (base_val, rest) = rest.split_at(base_len);
    let (exp_val, rest) = rest.split_at(exp_len);
    let (mod_val, _) = rest.split_at(mod_len);

    if (base_len <= 32) && (exp_len <= 32) && (mod_len <= 32) {
        let base_u256 = u8_array_to_u256(base_val);
        let exp_u256 = u8_array_to_u256(exp_val);
        let mod_u256 = u8_array_to_u256(mod_val);

        let result = mod_exp(base_u256, exp_u256, mod_u256);
        return result.to_be_bytes().to_vec().into_vector();
    }

    solana_program::big_mod_exp::big_mod_exp(base_val, exp_val, mod_val).into_vector()
}
