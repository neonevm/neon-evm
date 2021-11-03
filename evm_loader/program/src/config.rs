//! CONFIG MODULE
#![allow(clippy::use_self,clippy::nursery)]

use const_format::formatcp;
use cfg_if::cfg_if;

use evm::{ U256 };
use crate::macrorules::{ str_as_bytes_len, neon_elf_param };
use crate::account_data::ACCOUNT_SEED_VERSION;
use crate::account_data::ACCOUNT_MAX_SIZE;

cfg_if! {
    if #[cfg(feature = "mainnet")] {

        const CHAIN_ID: u64 = 245022934;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "9kPRbbwKL5SYELF4cZqWWFmP88QkKys51DoaUBx8eK73",
                "BMp6gEnveANdvSvspESJUrNczuHz1GF5UQKjVLCkAZih",
                "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRzZ",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";
        }

    } else if #[cfg(feature = "testnet")] {

        const CHAIN_ID: u64 = 245022940;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "EJUKLLjBMhFnkonfn7wcThnHyDewmhVmG9sEuVP9cvF8",
                "6ndMCacBc69VXqgNbcW3BLk2am9oeUDZa6SgBjHozDPd",
                "GEsnEWcKapTk7cgRoixBvCDc7yYuhmoMjpJ2v7mvmsBZ",
                "G5397iLxoKKYgMkFfkYBhJYEtErD7ygz8APmH59H8FM6",
                "rDeo4nZPE2aWpBkqFXBH8ygh1cD63nEKZPiDrpmQad6",
                "8hipwtwcmRH3iypYModkYFNXYGUEbxvpfqRhxPxx5Amx",
                "4fvtx2gJYJVd4o6CQt8Bdnc7dg5p2cgnb8oNUs7BGdd5",
                "9EMY6Xx18hN39CnzM6D5y9vuPa3YJ5ttbWRPJp3SX1Qk",
                "EMgay3kYFzHSh9PruAeRHxuGmNdsRQ6yPxzSAtU7PF7N",
                "4s5hHKLrfF7mcjfgwsRKdkubnC2VtswGpR2XGTCJaz3M",
                "F3V1pCfk1ZNk7Sdyh9N1H5eMtJq9XfhHR83fF8qa41Vt",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "7SBdHNeF9FFYySEoszpjZXXQsAiwa5Lzpsz6nUJWusEx");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";
        }
        
    } else if #[cfg(feature = "devnet")] {

        const CHAIN_ID: u64 = 245022926;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "Fg4uzL4QDfL6x56YFUcJBJSK3PqV4yXoFmXzZQkxn2DK",
                "8Uh8Rp1FWBiaDejyrZZhRY448oeG7GwKUyPDufP2Xxu7",
                "6ndMCacBc69VXqgNbcW3BLk2am9oeUDZa6SgBjHozDPd",
                "GEsnEWcKapTk7cgRoixBvCDc7yYuhmoMjpJ2v7mvmsBZ",
                "G5397iLxoKKYgMkFfkYBhJYEtErD7ygz8APmH59H8FM6",
                "rDeo4nZPE2aWpBkqFXBH8ygh1cD63nEKZPiDrpmQad6",
                "8hipwtwcmRH3iypYModkYFNXYGUEbxvpfqRhxPxx5Amx",
                "4fvtx2gJYJVd4o6CQt8Bdnc7dg5p2cgnb8oNUs7BGdd5",
                "9EMY6Xx18hN39CnzM6D5y9vuPa3YJ5ttbWRPJp3SX1Qk",
                "EMgay3kYFzHSh9PruAeRHxuGmNdsRQ6yPxzSAtU7PF7N",
                "4s5hHKLrfF7mcjfgwsRKdkubnC2VtswGpR2XGTCJaz3M",
                "F3V1pCfk1ZNk7Sdyh9N1H5eMtJq9XfhHR83fF8qa41Vt",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "7SBdHNeF9FFYySEoszpjZXXQsAiwa5Lzpsz6nUJWusEx");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";
        }
        
    } else {

        const CHAIN_ID: u64 = 111;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "9kPRbbwKL5SYELF4cZqWWFmP88QkKys51DoaUBx8eK73",
                "BMp6gEnveANdvSvspESJUrNczuHz1GF5UQKjVLCkAZih",
                "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRzZ",
            ]
        );
    
        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";
        }
        
    }
}

/// `PAYMENT_TO_COLLATERAL_POOL`
pub const PAYMENT_TO_COLLATERAL_POOL: u64 = 1000;
/// `PAYMENT_TO_DEPOSIT`
pub const PAYMENT_TO_DEPOSIT: u64 = 1000;
/// `OPERATOR_PRIORITY_SLOTS`
pub const OPERATOR_PRIORITY_SLOTS: u64 = 16;

neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_ACCOUNT_MAX_SIZE      , formatcp!("{:?}", ACCOUNT_MAX_SIZE));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", token_mint::DECIMALS));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
 }

