//! Faucet Ethereum utilities module.

use eyre::Result;

pub type Address = web3::types::Address;

/// Deletes the prefix 0x from a string representation of a hex number.
pub fn strip_0x_prefix(s: &str) -> &str {
    if s.len() < 3 || !s.starts_with("0x") {
        s
    } else {
        &s[2..]
    }
}

/// Converts string representation of address to the H160 hash format.
pub fn address_from_str(s: &str) -> Result<Address> {
    use std::str::FromStr as _;
    Ok(Address::from_str(strip_0x_prefix(s))?)
}

#[test]
fn test_address_from_str() {
    let r = address_from_str("ABC");
    assert!(r.is_err());
    assert_eq!(r.err().unwrap().to_string(), "Invalid input length");

    let r = address_from_str("ZYX");
    assert!(r.is_err());
    assert_eq!(
        r.err().unwrap().to_string(),
        "Invalid character 'Z' at position 0"
    );

    let r = address_from_str("0x00000000000000000000000000000000DeadBeef");
    assert!(r.is_ok());
}
