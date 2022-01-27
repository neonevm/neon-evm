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
                "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
                "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
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

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 1;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
           use crate::macrorules::{ str_as_bytes_len, neon_elf_param };

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "NeonPrG6tamsqnUwn1DEV9oi9e4JGbvSrgK6xKCiADf");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "NeonDdDx2MiiV3zwt5w1cDFii5Ru7TuKKh6p4Zjo3Ag");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "1");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "1");
        }

    } else if #[cfg(feature = "testnet")] {

        const CHAIN_ID: u64 = 245022940;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
                "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
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

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 1;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
           use crate::macrorules::{ str_as_bytes_len, neon_elf_param };

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "95tQS9NwHyboQm31za2FyNdxR8NVgqripwRUjZD97nrz");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "EqhCRgbZqCaXs6S8T2U2TJHkAffuNS99ot3ueFeUXJRF");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
        
    } else if #[cfg(feature = "devnet")] {

        const CHAIN_ID: u64 = 245022926;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
                "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
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
                "2GDfarSJnNC6ii5tQVE9rBH81Ny35LxrSCZ7tFhktSqi",
                "4Mh3ik4iS6MBxHy1VBN89vBiiPRDkebtnybDWnfTtpfC",
                "CyepBgaNezMJgLjy6Zyz9ECUia33dwDi9aXtRsZEhWX1",
                "HN4FeaSXB8t3FDW85hRw8mK1hYETJGeqhkkxJr6j2GiV",
                "5kKd1iy6onhCkzDq6DBw6woHLas3fy6HX4Yz8t1VPc1r",
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

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 10;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
           use crate::macrorules::{ str_as_bytes_len, neon_elf_param };

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "95tQS9NwHyboQm31za2FyNdxR8NVgqripwRUjZD97nrz");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "EqhCRgbZqCaXs6S8T2U2TJHkAffuNS99ot3ueFeUXJRF");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
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

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 10;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
           use crate::macrorules::{ str_as_bytes_len, neon_elf_param };

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "B2m2PGZQuZzaVMkeH8fLR8EbefiEy64ybCxVuzhx6RD1");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "D73ziEn1qS4egcMfADTZJnnn5XCENdcrDDcwAnSEvqGX");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
    }
}

/// `PAYMENT_TO_COLLATERAL_POOL`
pub const PAYMENT_TO_TREASURE: u64 = 5000;
/// `PAYMENT_TO_DEPOSIT`
pub const PAYMENT_TO_DEPOSIT: u64 = 5000;
/// `OPERATOR_PRIORITY_SLOTS`
pub const OPERATOR_PRIORITY_SLOTS: u64 = 16;

neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_ACCOUNT_MAX_SIZE      , formatcp!("{:?}", ACCOUNT_MAX_SIZE));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", token_mint::DECIMALS));
neon_elf_param!( NEON_PAYMENT_TO_TREASURE   , formatcp!("{:?}", PAYMENT_TO_TREASURE));
neon_elf_param!( NEON_PAYMENT_TO_DEPOSIT    , formatcp!("{:?}", PAYMENT_TO_DEPOSIT));
neon_elf_param!( NEON_CHAIN_ID              , formatcp!("{:?}", CHAIN_ID));
neon_elf_param!( NEON_POOL_COUNT            , formatcp!("{:?}", collateral_pool_base::NEON_POOL_COUNT));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
 }

