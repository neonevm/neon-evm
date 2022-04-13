//! CONFIG MODULE
#![allow(clippy::useless_transmute)]

use const_format::formatcp;
use cfg_if::cfg_if;
use evm::U256;

use crate::config_macro::{ neon_elf_param, declare_param_id, pubkey_array };
use crate::account::ACCOUNT_SEED_VERSION;

cfg_if! {
    if #[cfg(feature = "mainnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245_022_934;

        // NOTE: when expanding this list, add same addresses to the 
        // alpha configuration as well
        pubkey_array!(
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
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "NeonTjSjsuo3rexg9o6vHuMXw62f9V7zvmu8M8Zut44");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, "F4BYoes7Y6rs38QjNGC8F55bbohqt7G5qjzjDkzM4fiY");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 128;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "NeonPrG6tamsqnUwn1DEV9oi9e4JGbvSrgK6xKCiADf");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "NeonDdDx2MiiV3zwt5w1cDFii5Ru7TuKKh6p4Zjo3Ag");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "1");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "1");
        }

    } else if #[cfg(feature = "alpha")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245_022_923;

        pubkey_array!(
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
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "NeonTjSjsuo3rexg9o6vHuMXw62f9V7zvmu8M8Zut44");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, "F4BYoes7Y6rs38QjNGC8F55bbohqt7G5qjzjDkzM4fiY");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 128;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "NeonPrG6tamsqnUwn1DEV9oi9e4JGbvSrgK6xKCiADf");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "NeonDdDx2MiiV3zwt5w1cDFii5Ru7TuKKh6p4Zjo3Ag");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "1");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "1");
        }

    } else if #[cfg(feature = "testnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245_022_940;

        pubkey_array!(
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
                "Cpm5i9G1gLCDe9qm2y8coQquyGBQHfu8NgiC3JJnTeev",
                "813PRjWaqP2ZnirmLNgTL4xyC6yG5WoNEf8sihYnXSHU",
                "4sC1XfzkkKT67AKs2zwWJy7VEXcECger4an4s2F16JrK",
                "AjfMR1YetbbxYVpdR8uw9hR1pipFydnQy9qHDEM5cjRK",
                "2tZkAVEqYwtPDQrdSkbHUmXcD6UuSaWw7RBUSjVXi1s4",
                "HzzP7Gc5nKqKpro6Wj99ZDPAwyzGmwdXaQqzJ38XqFtf",
                "84qEuwNomqeC7wZZorLQFNj3XdPEycyaNXhVF6A4ThCw",
                "9KWDzP7m7FDhY6uTFNMfuSofLEeH3yiq1Zm9vvkNrp4E",
                "7nTeGU8UTtDgwj82qFGfp3Ug2ovnrD8Svwn4uygzwtVZ",
                "G4WHF5RvK346SWioD9jUk2aotsNjdXRoKwecUXac8Lcr",
                "5G5XRrtNhGEam6Dso4eynUctG6pSHBcyADGaWDPMZUZo",
                "8a7Yg3gqMARoH8Dp5K4QTETrfHHgzfqcprbjxaT9WwjT",
                "AcbET6BaNixJJSbVsSTMHqNbtmD29QcdmG8xDFDZhmAQ",
                "ETXp6z3GQuksC7fVbwr3dP7HC6KhDang2BqpwS5yEq4S",
                "72SengeGQD7XSdSXi6XnjvQwYpceWKqS2HmPKsvr3m3x",
                "6vYNpJXQywhEjapfXSm2GbuFGUjtRamntEn6YT3fUFJw",
                "j8Y4abKyAUhhvekN89c7EkYKVmUo5WPuVU3rgaJYivW",
                "9A7nYK1iBQyAcF1Hw1aRwcH3qR8pgGvM9VQ7hM9ii7DU",
                "1W6Z2oWehbpAK4AHvtEhXbqPQPyZpTKT74SMS4hGd39",
                "4RQdLBbbsv3mYTgAzxZLfq7gBNbY94mskVNE5nJzWEDG",
                "E7gcSAYWsFyKj7V1Rfqw4LazaPXQTabGThVNkVNuWM1m",
                "DR1UDBVEgMTaGwWjaQNzmtMATaucwmBbmCYQkoZ23bXx",
                "FrKZBETKPK2EUbyvSpdt61vDCGeUCuvoQdUd6GahZaoP",
                "FRvykJX7e7wZWy5E7yM1gcHHQZGUvk5hesM772KcKxyD",
                "DwJsWQQSBFcquyoUGKuCy12YAbf6xDQrct88ovTLbUts",
                "AAaNPfMcYswFCLuKhzs7ViAgQpvRcTJKP3MC5SraQVrV",
                "7edn5oDfjYBEXmp4vie3ywff1icG53dudrfenwpvCzQR",
                "5nEkR86At3fqKPNmvtPiwsgtricuhUknSdztoGuXgwCP",
                "H4eZrs754K3Dbrs23pWaKHpoYNNGdaKqBRmVKY5mhEmH",
                "8Dy9Nqtaj4kcHUWaXfKcpxuqJgvK85RefJXfKH331oF1",
                "9CNiUPsG3N7HApEgCdf29jhP1DCPRWhbwX8W8KAYGg3N",
                "35ZZAHPv15YVYRayYp79D4byy4bhrGEGm3QZVox69iaZ",
                "5dXnBiSUmidcYvsjL4QcX9MrPANSQ9NmhknCRoKNLmEw",
                "6zc5dTjN2Bur8j9t2Py8HV79R2dnEFBNegzSjFjgZNVL",
                "38iUn8t1wi5NBLtkndeTbKAmDN3DUzGSovZchJwAfk5a",
                "6EdzJ4WAYt5bbPFp62CEs6WzDRxiVYouGs2tVDKoyikJ",
                "7taK2nEXhZxxYA5sPrRKcEDZppztVyMEYeUnmNjoHyho",
                "FGRpARj8VNjK8wpsY2pDpmYmemjmoAPUPFqWpFYw9Z5",
                "CtK57wnaahbnj4kHcRTqK3GqHUm2u3fgoWWeQoWgKLuc",
                "4Cp4tbVspRVTdicH5L9ofbsDENbYGjV9nS1kVmBZJZi6",
                "Hkyk6XoFgSX8pccS4SzsAgroCPg7hhqSDPGZS9N3ib7t",
                "5mWoAtX7Ge2Sk4aoyLXHhayGemQAnkugJjU65teC1nCF",
                "7r387NaDsWai3JWoRMA7oboNpJdJsqp9ZrHmBFDoAdVs",
                "Gds34TkCQK6Cn5UY6Ua63FLA4zHJVL2QnkpzytECsR8Y",
                "4bpTD1CjhPj6k6JUiRhFtvGRAprPEaQzMpwnvhdgNhco",
                "4aDDAQoHH7EqRdKx3WChJE2X46bYoQu1omyga1AbSK4C",
                "6UwcFs1XYr9t3Rb8Us577HnD8VFP7Y27SqycCmHe3hdZ",
                "6KqN7yRn3e1VQsN8T4daSjGw8VWA1itfWtMcaVCtViwA",
                "3Kc7GwRzz6gE9CyvkR5M9pJDegCewKSf97aqhNLwj1T1",
                "2P3yYwJtvcDPG1FkjMa2ZRx5CcfU4BE1eBLxbA4RjwaH",
                "35vZqRVJwzETutp9qfrQSVAbfdEdCaTTeNMWFHmTcU9k",
                "6C4fqJfP4mBPVQGYGeswi6NMpguHB1Z3V6CB6swNAgQ8",
                "GGpZz5Pgk5ZK3MewVmsgN3K3q8ELV1S6G9EqGCp9Fusk",
                "5aNR2vRnkeRbRaJ5m6u65ozJkcbKUF3CuAWAd7wcc4VL",
                "39ZW3JfejGmKPWMt5mCHDrdXbr2Zqa693PTSw8CF6Hiv",
                "6N639L8KEYtXzuK6S2s3igQEpWV9NPRmMUH27EuanoRC",
                "4SgQqMMeqkfRxF7XosHZPavhMsQChKbKFgdA5gbqKK69",
                "2pwajL5zgaypeLW3iwqgg8Q34k8cbF6FuH7hbviQGHsn",
                "Bwcf3tPB7ARgq6jYH1mCA8na14azGThNNP9U9yBuNaNK",
                "3bdkShjGK9BdSsmzvFqJ4KYMXEEYSiHppXRm8CAKRLRG",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, "7SBdHNeF9FFYySEoszpjZXXQsAiwa5Lzpsz6nUJWusEx");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 10;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "95tQS9NwHyboQm31za2FyNdxR8NVgqripwRUjZD97nrz");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "EqhCRgbZqCaXs6S8T2U2TJHkAffuNS99ot3ueFeUXJRF");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
        
    } else if #[cfg(feature = "devnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245_022_926;

        pubkey_array!(
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
                "Cpm5i9G1gLCDe9qm2y8coQquyGBQHfu8NgiC3JJnTeev",
                "813PRjWaqP2ZnirmLNgTL4xyC6yG5WoNEf8sihYnXSHU",
                "4sC1XfzkkKT67AKs2zwWJy7VEXcECger4an4s2F16JrK",
                "AjfMR1YetbbxYVpdR8uw9hR1pipFydnQy9qHDEM5cjRK",
                "2tZkAVEqYwtPDQrdSkbHUmXcD6UuSaWw7RBUSjVXi1s4",
                "HzzP7Gc5nKqKpro6Wj99ZDPAwyzGmwdXaQqzJ38XqFtf",
                "84qEuwNomqeC7wZZorLQFNj3XdPEycyaNXhVF6A4ThCw",
                "9KWDzP7m7FDhY6uTFNMfuSofLEeH3yiq1Zm9vvkNrp4E",
                "7nTeGU8UTtDgwj82qFGfp3Ug2ovnrD8Svwn4uygzwtVZ",
                "G4WHF5RvK346SWioD9jUk2aotsNjdXRoKwecUXac8Lcr",
                "5G5XRrtNhGEam6Dso4eynUctG6pSHBcyADGaWDPMZUZo",
                "8a7Yg3gqMARoH8Dp5K4QTETrfHHgzfqcprbjxaT9WwjT",
                "AcbET6BaNixJJSbVsSTMHqNbtmD29QcdmG8xDFDZhmAQ",
                "ETXp6z3GQuksC7fVbwr3dP7HC6KhDang2BqpwS5yEq4S",
                "72SengeGQD7XSdSXi6XnjvQwYpceWKqS2HmPKsvr3m3x",
                "6vYNpJXQywhEjapfXSm2GbuFGUjtRamntEn6YT3fUFJw",
                "j8Y4abKyAUhhvekN89c7EkYKVmUo5WPuVU3rgaJYivW",
                "9A7nYK1iBQyAcF1Hw1aRwcH3qR8pgGvM9VQ7hM9ii7DU",
                "1W6Z2oWehbpAK4AHvtEhXbqPQPyZpTKT74SMS4hGd39",
                "4RQdLBbbsv3mYTgAzxZLfq7gBNbY94mskVNE5nJzWEDG",
                "E7gcSAYWsFyKj7V1Rfqw4LazaPXQTabGThVNkVNuWM1m",
                "DR1UDBVEgMTaGwWjaQNzmtMATaucwmBbmCYQkoZ23bXx",
                "FrKZBETKPK2EUbyvSpdt61vDCGeUCuvoQdUd6GahZaoP",
                "FRvykJX7e7wZWy5E7yM1gcHHQZGUvk5hesM772KcKxyD",
                "DwJsWQQSBFcquyoUGKuCy12YAbf6xDQrct88ovTLbUts",
                "AAaNPfMcYswFCLuKhzs7ViAgQpvRcTJKP3MC5SraQVrV",
                "7edn5oDfjYBEXmp4vie3ywff1icG53dudrfenwpvCzQR",
                "5nEkR86At3fqKPNmvtPiwsgtricuhUknSdztoGuXgwCP",
                "H4eZrs754K3Dbrs23pWaKHpoYNNGdaKqBRmVKY5mhEmH",
                "8Dy9Nqtaj4kcHUWaXfKcpxuqJgvK85RefJXfKH331oF1",
                "9CNiUPsG3N7HApEgCdf29jhP1DCPRWhbwX8W8KAYGg3N",
                "35ZZAHPv15YVYRayYp79D4byy4bhrGEGm3QZVox69iaZ",
                "5dXnBiSUmidcYvsjL4QcX9MrPANSQ9NmhknCRoKNLmEw",
                "6zc5dTjN2Bur8j9t2Py8HV79R2dnEFBNegzSjFjgZNVL",
                "38iUn8t1wi5NBLtkndeTbKAmDN3DUzGSovZchJwAfk5a",
                "6EdzJ4WAYt5bbPFp62CEs6WzDRxiVYouGs2tVDKoyikJ",
                "7taK2nEXhZxxYA5sPrRKcEDZppztVyMEYeUnmNjoHyho",
                "FGRpARj8VNjK8wpsY2pDpmYmemjmoAPUPFqWpFYw9Z5",
                "CtK57wnaahbnj4kHcRTqK3GqHUm2u3fgoWWeQoWgKLuc",
                "4Cp4tbVspRVTdicH5L9ofbsDENbYGjV9nS1kVmBZJZi6",
                "Hkyk6XoFgSX8pccS4SzsAgroCPg7hhqSDPGZS9N3ib7t",
                "5mWoAtX7Ge2Sk4aoyLXHhayGemQAnkugJjU65teC1nCF",
                "7r387NaDsWai3JWoRMA7oboNpJdJsqp9ZrHmBFDoAdVs",
                "Gds34TkCQK6Cn5UY6Ua63FLA4zHJVL2QnkpzytECsR8Y",
                "4bpTD1CjhPj6k6JUiRhFtvGRAprPEaQzMpwnvhdgNhco",
                "4aDDAQoHH7EqRdKx3WChJE2X46bYoQu1omyga1AbSK4C",
                "6UwcFs1XYr9t3Rb8Us577HnD8VFP7Y27SqycCmHe3hdZ",
                "6KqN7yRn3e1VQsN8T4daSjGw8VWA1itfWtMcaVCtViwA",
                "3Kc7GwRzz6gE9CyvkR5M9pJDegCewKSf97aqhNLwj1T1",
                "2P3yYwJtvcDPG1FkjMa2ZRx5CcfU4BE1eBLxbA4RjwaH",
                "35vZqRVJwzETutp9qfrQSVAbfdEdCaTTeNMWFHmTcU9k",
                "6C4fqJfP4mBPVQGYGeswi6NMpguHB1Z3V6CB6swNAgQ8",
                "GGpZz5Pgk5ZK3MewVmsgN3K3q8ELV1S6G9EqGCp9Fusk",
                "5aNR2vRnkeRbRaJ5m6u65ozJkcbKUF3CuAWAd7wcc4VL",
                "39ZW3JfejGmKPWMt5mCHDrdXbr2Zqa693PTSw8CF6Hiv",
                "6N639L8KEYtXzuK6S2s3igQEpWV9NPRmMUH27EuanoRC",
                "4SgQqMMeqkfRxF7XosHZPavhMsQChKbKFgdA5gbqKK69",
                "2pwajL5zgaypeLW3iwqgg8Q34k8cbF6FuH7hbviQGHsn",
                "Bwcf3tPB7ARgq6jYH1mCA8na14azGThNNP9U9yBuNaNK",
                "3bdkShjGK9BdSsmzvFqJ4KYMXEEYSiHppXRm8CAKRLRG",
            ]
        );

        /// Token Mint ID
        pub mod token_mint {
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "89dre8rZjLNft7HoupGiyxu3MNftR577ZYu8bHe2kK7g");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, "7SBdHNeF9FFYySEoszpjZXXQsAiwa5Lzpsz6nUJWusEx");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 10;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "95tQS9NwHyboQm31za2FyNdxR8NVgqripwRUjZD97nrz");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "EqhCRgbZqCaXs6S8T2U2TJHkAffuNS99ot3ueFeUXJRF");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
        
    } else {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 111;

        pubkey_array!(
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
		"2Ma3MxGpKmk2KPbp631bNhm2NcSMU6oxFgtj2FfzkiBF",
		"2v3dnQQaBALRmaQ1Jr7GbCVagTqEBKHPZ65b4nAmdDmN",
		"47dYMgKdKxRGuGBpjH58eGuj1n4FXC6v4QTcpCSaVC2c",
		"5dyQQATyk4yga4f4m8BCrUF1jdfGQ1mShV4ezFLxyCqW",
		"7C6iuRYzEJEwe878X2TeMDoCHPEw85ZhaxapNEBuqwL9",
		"82YcsM5eN83trdhdShGUF4crAC4CGgFJ7EWd2vnGiSsb",
		"A3CEBvqJPPgHPARxzUQUafHXC4iU6x4iZzNudJ1Tks4z",
		"AezpxgT4Qbo1pB9cLgBzzET7V2t7yK2ZrJrhDTCwxac9",
		"CXJy6dzL8kAazo5jhBf8MuW17nJ8dW23EfzPmqTJ6P5H",
		"DPRfsB8HQrJZM5g3B74rqZSmvtJn41PavhKBjmCRb45R",
		"EbkUFw2EQkG85ua4sQy54Y6c988j7zkSAjkD6gRUTA3u",
		"F4nLmDy62mhYiY4gGmRXDYpdFM4mLrm9t5YLpqTDMBz5",
		"GHGLwKXzo2fAtLAVNJisP7wNyCRWBcmHEzCD36UcutW1",
		"GZ3vKajaDjxFkiczL4g6as3qhMg7tdMgrMrpuApGWF8D",
		"eXiURdoUQ4JpUysAevcTPiLMdWwG8q6mRAmice5Kioh",
            ]
        );
    
        /// Token Mint ID
        pub mod token_mint {
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "HPsV9Deocecw3GeZv1FkAPNCBRfuVyfw9MMwjwRe1xaU");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, "4sW3SZDJB7qXUyCYKA7pFL8eCTfm3REr8oSiKkww7MaT");

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = "collateral_seed_";

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = 10;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

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
pub const HOLDER_MSG_SIZE: u64 = 950;
/// `OPERATOR_PRIORITY_SLOTS`
pub const COMPUTE_BUDGET_UNITS: u32 = 500_000;
/// `OPERATOR_PRIORITY_SLOTS`
pub const COMPUTE_BUDGET_HEAP_FRAME: u32 = 256 * 1024;
/// Additional fee for `request units` instruction
pub const REQUEST_UNITS_ADDITIONAL_FEE: u32 = 0;
/// Gas limit multiplier for transactions without chain id
pub const GAS_LIMIT_MULTIPLIER_NO_CHAINID: u32 = 100;

neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", token_mint::DECIMALS));
neon_elf_param!( NEON_PAYMENT_TO_TREASURE   , formatcp!("{:?}", PAYMENT_TO_TREASURE));
neon_elf_param!( NEON_PAYMENT_TO_DEPOSIT    , formatcp!("{:?}", PAYMENT_TO_DEPOSIT));
neon_elf_param!( NEON_CHAIN_ID              , formatcp!("{:?}", CHAIN_ID));
neon_elf_param!( NEON_POOL_COUNT            , formatcp!("{:?}", collateral_pool_base::NEON_POOL_COUNT));
neon_elf_param!( NEON_HOLDER_MSG_SIZE       , formatcp!("{:?}", HOLDER_MSG_SIZE));
neon_elf_param!( NEON_COMPUTE_UNITS         , formatcp!("{:?}", COMPUTE_BUDGET_UNITS));
neon_elf_param!( NEON_HEAP_FRAME            , formatcp!("{:?}", COMPUTE_BUDGET_HEAP_FRAME));
neon_elf_param!( NEON_ADDITIONAL_FEE        , formatcp!("{:?}", REQUEST_UNITS_ADDITIONAL_FEE));
neon_elf_param!( NEON_GAS_LIMIT_MULTIPLIER_NO_CHAINID, formatcp!("{:?}", GAS_LIMIT_MULTIPLIER_NO_CHAINID));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
}

