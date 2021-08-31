//! Faucet Ethereum utilities module.

use color_eyre::Result;

pub type Address = web3::types::Address;

/// Converts string representation of address to the H160 hash format.
pub fn address_from_str(s: &str) -> Result<Address> {
    use std::str::FromStr as _;
    let address = if !s.starts_with("0x") {
        Address::from_str(s)?
    } else {
        Address::from_str(&s[2..])?
    };
    Ok(address)
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
