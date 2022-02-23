//! CONFIG MODULE
#![allow(clippy::use_self,clippy::nursery)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]

use const_format::formatcp;
use cfg_if::cfg_if;

use evm::{ U256 };
use crate::macrorules::{ str_as_bytes_len, neon_elf_param };
use crate::account::ACCOUNT_SEED_VERSION;

cfg_if! {
    if #[cfg(feature = "mainnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245022934;

        // NOTE: when expanding this list, add same addresses to the 
        // alpha configuration as well
        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "NeonPQFrw5stVvs1rFLDxALWUBDCnSPsWBP83RfNUKK",
                "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
                "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
                "DSRVyWpSVLEcHih9CVND2aGNBZxNW5bt34GEaK4aDk5i",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "NeonTjSjsuo3rexg9o6vHuMXw62f9V7zvmu8M8Zut44");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "F4BYoes7Y6rs38QjNGC8F55bbohqt7G5qjzjDkzM4fiY");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 128;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
           use crate::macrorules::{ str_as_bytes_len, neon_elf_param };

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "NeonPrG6tamsqnUwn1DEV9oi9e4JGbvSrgK6xKCiADf");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "NeonDdDx2MiiV3zwt5w1cDFii5Ru7TuKKh6p4Zjo3Ag");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "1");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "1");
        }

    } else if #[cfg(feature = "alpha")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245022923;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "NeonPQFrw5stVvs1rFLDxALWUBDCnSPsWBP83RfNUKK",
                "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
                "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
                "DSRVyWpSVLEcHih9CVND2aGNBZxNW5bt34GEaK4aDk5i",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_TOKEN_MINT, "NeonTjSjsuo3rexg9o6vHuMXw62f9V7zvmu8M8Zut44");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use crate::macrorules::{ str_as_bytes_len, neon_elf_param, declare_param_id };

            declare_param_id!(NEON_POOL_BASE, "F4BYoes7Y6rs38QjNGC8F55bbohqt7G5qjzjDkzM4fiY");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 128;
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

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245022940;

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
                "AqwN5pPsf9pnUZUSo7SmELRrBxxFnycEnTx5spVji4R6",
                "FtFt7sMNfPUTWKx3otH4aor4KWoFdk9p5qSyxmSj4ZVH",
                "FMMshJoSaNaNFdHseaWAgvHTypS4zggr3fpqEa1FPqYT",
                "2S6YTfDmk3PMZUorMqkCRM8zJqTrMtzu8x5eo1YboMGg",
                "72jAG5diJkivWJ2Var2SFuYK2P2vjxaZ2wEUSR23GX7a",
                "B5Cwn8y3JaFV622wdkocccJ3U1rfjCWA4S922x2ujLU5",
                "JCjvNTNTfZeo9mSUB4kBVKCJFGiMm4Hux2DSLFubrgVW",
                "D1apcJxXxAS63cpbTidxjXku7cW2ELQQU9szMQracDSY",
                "cSqdT68gjm4JBS67268wxgx5nQ1G8nZBZWLq8Cu12kM",
                "2xG1HNtGyJU7toexYdZZmXUnkb1Sf9fuNPtUycieKqDu",
                "Gom4mLPW9yCe1gpAGExR89KvH8je1mExxhuEHTPDm7HP",
                "AnKcUi9sRv1LwhKwW8HePfFYQM88wppXPYsE7kzL7DMA",
                "GV4hrkRD4FnRySu1QrAaepSVW3bcNaZ8Qzi3s8aFU8rX",
                "GqL8pvEzwCpJrQG4QvhkVqVuGjt88aX2K4hJYCNZ17MM",
                "DdGh2cRow4Mox55bpp9LSzX81e8jvivfnn5U4dVgcVw9",
                "AtE89m4yMfQ5kkJoJip3zmYWYH3KKcKirdsX7xQAqxKR",
                "CoZPFRcnaYYpxcKQaQ1PmL19qdn2UvpNWrNhT2mgeUSj",
                "4pNmbWw9jKK5FwXenyUWBFPH34tDT8pPFhxveDT45oKU",
                "2hGSQxwTVefwmD4ta8XbDS2Mst7JLCWBsPv4sF23UU4f",
                "7YhQwogejtqmDNDSeabQeVmaxZsTWtXGsbbYUErpbP3L",
                "AoJ9MPwwfdgognNy6AphcWvmp7NFpJR3dBwQBgyTUvqi",
                "BJ3dVNC6rmL4fLdxWD2kjcokF41gT2RoAFydbCbCthzH",
                "GZz2xY4UvRz1Rqcx4wwf8416x2SuQGvqawkUqsWVeCxD",
                "ATNYDjFne7E41K2gWq6WhkkXMVm8sVyvYgDuXpUn6XGa",
                "53wqLPWxMqTtrF9JzJyJMzzjou5ELYbHzizkReE9ReN1",
                "E3Y1hJpMv2wddU1SxTLKz5R5S4P4ZgeZ6Mo4e8Uurqsy",
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
        
    } else if #[cfg(feature = "devnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245022926;

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
                "AqwN5pPsf9pnUZUSo7SmELRrBxxFnycEnTx5spVji4R6",
                "FtFt7sMNfPUTWKx3otH4aor4KWoFdk9p5qSyxmSj4ZVH",
                "FMMshJoSaNaNFdHseaWAgvHTypS4zggr3fpqEa1FPqYT",
                "2S6YTfDmk3PMZUorMqkCRM8zJqTrMtzu8x5eo1YboMGg",
                "72jAG5diJkivWJ2Var2SFuYK2P2vjxaZ2wEUSR23GX7a",
                "B5Cwn8y3JaFV622wdkocccJ3U1rfjCWA4S922x2ujLU5",
                "JCjvNTNTfZeo9mSUB4kBVKCJFGiMm4Hux2DSLFubrgVW",
                "D1apcJxXxAS63cpbTidxjXku7cW2ELQQU9szMQracDSY",
                "cSqdT68gjm4JBS67268wxgx5nQ1G8nZBZWLq8Cu12kM",
                "2xG1HNtGyJU7toexYdZZmXUnkb1Sf9fuNPtUycieKqDu",
                "Gom4mLPW9yCe1gpAGExR89KvH8je1mExxhuEHTPDm7HP",
                "AnKcUi9sRv1LwhKwW8HePfFYQM88wppXPYsE7kzL7DMA",
                "GV4hrkRD4FnRySu1QrAaepSVW3bcNaZ8Qzi3s8aFU8rX",
                "GqL8pvEzwCpJrQG4QvhkVqVuGjt88aX2K4hJYCNZ17MM",
                "DdGh2cRow4Mox55bpp9LSzX81e8jvivfnn5U4dVgcVw9",
                "AtE89m4yMfQ5kkJoJip3zmYWYH3KKcKirdsX7xQAqxKR",
                "CoZPFRcnaYYpxcKQaQ1PmL19qdn2UvpNWrNhT2mgeUSj",
                "4pNmbWw9jKK5FwXenyUWBFPH34tDT8pPFhxveDT45oKU",
                "2hGSQxwTVefwmD4ta8XbDS2Mst7JLCWBsPv4sF23UU4f",
                "7YhQwogejtqmDNDSeabQeVmaxZsTWtXGsbbYUErpbP3L",
                "AoJ9MPwwfdgognNy6AphcWvmp7NFpJR3dBwQBgyTUvqi",
                "BJ3dVNC6rmL4fLdxWD2kjcokF41gT2RoAFydbCbCthzH",
                "GZz2xY4UvRz1Rqcx4wwf8416x2SuQGvqawkUqsWVeCxD",
                "ATNYDjFne7E41K2gWq6WhkkXMVm8sVyvYgDuXpUn6XGa",
                "53wqLPWxMqTtrF9JzJyJMzzjou5ELYbHzizkReE9ReN1",
                "E3Y1hJpMv2wddU1SxTLKz5R5S4P4ZgeZ6Mo4e8Uurqsy",
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

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 111;

        macros::pubkey_array!(
            AUTHORIZED_OPERATOR_LIST,
            [
                "9kPRbbwKL5SYELF4cZqWWFmP88QkKys51DoaUBx8eK73",
                "BMp6gEnveANdvSvspESJUrNczuHz1GF5UQKjVLCkAZih",
                "CVAimMqtcmSUCV4RLZSJAreDpEd7JEZmrvCVj85yaRzZ",
                "5mszzfV23zRfcAdn9d7kmW6Qn57SGkpGywyqyVCVc817",
                "AdtXr9yGAsTokY75WernsmQdcBPu2LE2Bsh8Nx3ApbbR",
                "2k8NURTZ8xd1qD2JhujP2MjxkLgLHUmwpXP8FNSP7ntd",
                "EkEBZJmw4uKfvruf3c6EFJeWeAY3rev3hRsp2S4BDV3M",
                "9LosHtRuxtFYtj2bJfvkcZpDywtdktpAabRQ7hCQasmt",
                "FHYUtkDhNaMdaKzP2y7ZXmy6HtiKz7uahz19CPUkjeiD",
                "3NqgsSRfjpmDfzRH4PLKrzBvMc8MgFXgU58Yy8n41KF5",
                "8HStt6KLgRY8CBNmDjwMTZhCFrXeVrEgVp3jTS4gaRYR",
                "V6fr3EgLUYFSGgzPBxTnhrieeAratBX46AGvAtmQ2Xe",
                "7r5GAh4SDhBwxg98vT86Q8sA8c9zEgJduSWWCV1y48V",
                "GwUnjJs6i7TKGjy71PvFpGN7yu9xqA8Cs1oyV4zSVPvq",
                "EdSEh9UxXjbrrHLrH5manpxfXi7HxzkAMDAotPC5DggQ",
                "9s7umnvnGqT1nvrCgzvBwWFyaaYABj64LxiBpjAayLiv",
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
/// `the message size that is used to holder-account filling`
pub const HOLDER_MSG_SIZE: u64 = 1000;


neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", token_mint::DECIMALS));
neon_elf_param!( NEON_PAYMENT_TO_TREASURE   , formatcp!("{:?}", PAYMENT_TO_TREASURE));
neon_elf_param!( NEON_PAYMENT_TO_DEPOSIT    , formatcp!("{:?}", PAYMENT_TO_DEPOSIT));
neon_elf_param!( NEON_CHAIN_ID              , formatcp!("{:?}", CHAIN_ID));
neon_elf_param!( NEON_POOL_COUNT            , formatcp!("{:?}", collateral_pool_base::NEON_POOL_COUNT));
neon_elf_param!( NEON_HOLDER_MSG_SIZE       , formatcp!("{:?}", HOLDER_MSG_SIZE));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
 }

