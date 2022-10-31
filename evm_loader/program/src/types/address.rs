use std::convert::{From, TryInto};
use std::fmt::{Display, Debug};
use serde::{Serialize, Deserialize};
use hex::FromHex;
use solana_program::pubkey::Pubkey;

use crate::account::ACCOUNT_SEED_VERSION;
use crate::error::Error;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Address(pub [u8; 20]);


impl Address {
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    #[must_use]
    pub fn from_create(source: &Address, nonce: u64) -> Self {
        use solana_program::keccak::{hash, Hash};

        let mut stream = rlp::RlpStream::new_list(2);
        stream.append(source);
        stream.append(&nonce);
        let Hash(hash) = hash(&stream.out());

        let bytes = arrayref::array_ref![hash, 12, 20];
        Address(*bytes)
    }

    #[must_use]
    pub fn from_create2(source: &Address, salt: &[u8; 32], initialization_code: &[u8]) -> Self {
        use solana_program::keccak::{hash, hashv, Hash};

        let Hash(code_hash) = hash(initialization_code);
        let Hash(hash) = hashv(&[ &[0xFF], source.as_bytes(), salt, &code_hash ]);

        let bytes = arrayref::array_ref![hash, 12, 20];
        Address(*bytes)
    }

    pub fn from_hex(mut s: &str) -> Result<Self, Error> {
        if s.starts_with("0x") {
            s = &s[2..];
        }

        let bytes = <[u8; 20]>::from_hex(s)?;
        Ok(Address(bytes))
    }

    #[must_use]
    pub fn find_solana_address(&self, program_id: &Pubkey) -> (Pubkey, u8) {
        let seeds: &[&[u8]] = &[&[ACCOUNT_SEED_VERSION], &self.0];
        Pubkey::find_program_address(seeds, program_id)
    }
}

impl From<[u8; 20]> for Address {
    fn from(value: [u8; 20]) -> Self {
        Self(value)
    }
}

impl From<Address> for [u8;20] {
    fn from(value: Address) -> Self {
        value.0
    }
}


impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex_string = hex::encode(self.0);
        f.write_str("0x")?;
        f.write_str(&hex_string)
    }
}

impl Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex_string = hex::encode(self.0);
        f.write_str("0x")?;
        f.write_str(&hex_string)
    }
}


impl rlp::Encodable for Address {
    fn rlp_append(&self, stream: &mut rlp::RlpStream) {
        let Address(bytes) = self;
        stream.encoder().encode_value(bytes);
    }
}

impl rlp::Decodable for Address {
    fn decode(rlp: &rlp::Rlp) -> Result<Self, rlp::DecoderError> {
        rlp.decoder().decode_value(|bytes| {
            let array: [u8; 20] = bytes.try_into().map_err(|_| rlp::DecoderError::RlpInvalidLength)?;
            Ok(Address(array))
        })
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer 
    {
        if serializer.is_human_readable() {
            self.to_string().serialize(serializer)
        } else {
            serializer.serialize_bytes(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> 
    {
        struct AddressVisitor;

        impl<'de> serde::de::Visitor<'de> for AddressVisitor
        {
            type Value = Address;
        
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Ethereum Address")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                where
                    E: serde::de::Error, 
            {
                let address = Address::from_hex(v)
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Str(v), &self))?;
                    
                Ok(address)
            }
        
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let bytes = v
                    .try_into()
                    .map_err(|_| E::invalid_length(v.len(), &self))?;
        
                Ok(Address(bytes))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(AddressVisitor)
        } else {
            deserializer.deserialize_bytes(AddressVisitor)
        }
    }
}
