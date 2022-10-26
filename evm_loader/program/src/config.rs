//! CONFIG MODULE
#![allow(clippy::useless_transmute)]

use const_format::formatcp;
use cfg_if::cfg_if;
use evm::U256;
use evm_loader_macro::{
    operators_whitelist,
    neon_elf_param,
    declare_param_id
};

use crate::account::ACCOUNT_SEED_VERSION;

/// Seed to generate PDA for treasury balances 
pub const TREASURY_POOL_SEED: &str = "treasury_pool";

/// Count of balances in treasury pool
pub const TREASURY_POOL_COUNT: u32 = 10000;

cfg_if! {
    if #[cfg(feature = "mainnet")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 245_022_934;

        // NOTE: when expanding this list, add same addresses to the 
        // alpha configuration as well
        operators_whitelist![
            "NeonPQFrw5stVvs1rFLDxALWUBDCnSPsWBP83RfNUKK",
            "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
            "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
            "DSRVyWpSVLEcHih9CVND2aGNBZxNW5bt34GEaK4aDk5i",
        ];

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

        operators_whitelist![
            "NeonPQFrw5stVvs1rFLDxALWUBDCnSPsWBP83RfNUKK",
            "NeoQM3utcHGxhKT41Nq81g8t4xGcPNFpkAgYj1N2N8v",
            "Gw3Xiwve6HdvpJeQguhwT23cpK9nRjSy1NpNYCFY4XU9",
            "DSRVyWpSVLEcHih9CVND2aGNBZxNW5bt34GEaK4aDk5i",
        ];

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

        operators_whitelist![
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
            "Cy2e827aiHG1YjPpeyhxdYLsv41GHRXGF6eXm5BhefoP",
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
            "AtrntfLuNFrqmmXsKGRiT7mYFeb5WhFvbNi5PeCwxqvK",
            "GduRLuJswRRZvC2bjFFFpeGVZyjxBv64gL3dRkT8H9WK",
            "2wGuhJ5J5qxQTyye5jkw97DG2riahzfS9TVcUrdKfYZE",
            "BftXuBxRV8sSigUk3BaYNR29d7jkGCEJ7H2pdJ5DsUs5",
            "3u9nAi7nGd9HSPNiTUoZ9Yeg2foUig4DyDjQCMZcUfuB",
            "D7yYMD3CAetZV68sfZyEYrjLEdTCy4euGHLMDDhbYqRC",
            "BgtZ5ERP1dk3pX6R3ZhLLsn9gK2712FFBJRwPLJzjf3Q",
            "GD5CtfkvLJgvSt5NhxYUvvTMP8J5njVZSjEZTgtFhByA",
            "FSWLCdQjiJcw4zFvqjdxSKtesLojygihKq2qRUcszfME",
            "86qxUGvmc5CbLrbT55W11Rxf9seSELTc2iwtZAMQCCN3",
            "5bFfsYH8mvuUkNYiJiXfRLPa66dHgFcqgtDpFsKYCGCR",
            "FLV73f4jgphZGtGqGyL88gRw145rAr9ab9HLia9WHaGa",
            "Eao4cfXw3sEVqixo18i6rVGRCbBA6HK4oth47pPkTwhF",
            "7zpspz9cf2V8sFdDMiKRWWdvmiUuX4FSgJTTVkARvos5",
            "GpobXosk2skR4SufcvN6BtAZzoCSZ2ah61uqtV3yUKvf",
            "ECjgJRuwj8KWhgaBqMwBVuvSMX3PdpbHkGmhgCBGgwmp",
            "DgyJUjow52EARe5qdQewgivgpYztUpjam1WwgxgfZSop",
            "CMXAcbx8CUKjrzGfGFXCEUFWy7pCk1DQ4nULb2sf4MfJ",
            "7rzKApzDv6unzj2qKwkAk9epQaqXPJU478oLkUpdmUDE",
            "6JnQMijtWU6Hd8Fq4TGknXkaUtsihc2LQXnyqeNvmm4V",
            "GfFStrfhsJMeKPjtUU2sbSPjZTqCe4V9e4KEpowCXQDw",
            "d3LuBwryPnEgSDX1xMV1EQbzSjz59dibvtpSknHkZWy",
            "8NHb7kThj3V4jn2FT6Qns44W3kgfAHRUp9YwiKU2iKki",
            "54WJZHA39L4UtCKTcVfY6hmLH3KfPTVrWWn9CdCozVUX",
            "2UcM2qimf1fhErp7AffzeaDAKw4uAVaL5Yf1mVmUwES1",
            "jdqK5gvjg5bCnaWTmkWhREwGWpyoz4Bf7D4zkCgq7Yr",
            "J7SCxdgTapRwUWbUqtot69kTn1YW9i3LYVpZ1CZRCktf",
            "4QzexgShewNN7vjyiGbkqgHWMmFMzQBsNy8eUFj3tueX",
            "HNBEm3pVcR2RUL2D2VGz12Fc5ABBy2sgsb6Hqw77ePGf",
            "1aPHVAsL84LFrn7AB3Gorn8hjd65FzWfp899edy62MA",
            "2gWBPL55eMW8YuNDYy6FnNhbjEDNVsXZM8xujMArkj7f",
            "9T7iuW4HqCcJWxjNcaUx2vcE67Mkpw61AWS6oYFg54oK",
            "Hc5Hndg1RXqBbGg387qUqD1eQSe4Ti6f6UWxRaFn7Csh",
            "Cso8JZBNUwZENUwKK4ggrT6dpHWY29tPJunYz7tQhGnc",
            "FBPvUsnYPvzcadGdrX9sMeqDgZeYC4knhFPUAWWGJbFV",
            "87YzMrZMgo2ujx5DCvyk1YJvwKcSvPgQ16niMSv4qBX8",
            "Fc6kL9hwZArmZggpkXUkwts2P124Uv34EbBjHCj7gdcU",
            "CWQWWPJgC5G1nJvuHKZ4R9paDca4v7kVbBw3zSBafm96",
            "2jFtMCy5biCFb2pTduaBBAFzTvQ1GsjXbP6FSaQXxYWG",
            "6aSPCHiYUpNVRQFJBT1FWvfdERA6M3VbtbeG1heg5aUJ",
            "2GR4jUxzDSnCzp5kui4Bp96xe6HDLA94E9esELvh4KdY",
            "2H694rNVMMw9RW4g2frbvjRKyu3EVfUpuTGRWCyxqmHR",
            "2o6WUWZrD2wufWD5i7d8WdkaCWkweguYcqQUQFSpuiy3",
            "3UfhW285qXsoXyBeSR7nL41f4t5omatFcXiLt5dE54R4",
            "4iYxXBHu227A57S2pWLysEnY4LiRzqStHmh3maBqi7UL",
            "4mNxgrH3my7jfiapanDV9bPTprrHzGS8v7iUY7tpiCym",
            "5MdtsuUHgkhXtRC3ChbbKbmWMsTQeCTewLVf2Ah5LrYx",
            "5V6pkMT9cCicGBvtxDJDQPUovopV4CDHnxBPxQ2CvjPt",
            "5ipjaDS6n2ZzHqU4qGQQFAcUNHgeapEDWE8r564DCpj5",
            "5swQcW8JgAxXMZqSev9mBC7Nt3PB4BiiSjjDe8PFrRmd",
            "6WUPSrVbyCHfv2iMFvVEtNzdMEePsZ7jetZLWUwmRGr8",
            "6ZcFvLtFxcaZ5PDfg25D4BZ41dvBMnhKF5eJPFxvmFZF",
            "6uCmKHoiDDG7GzmZGQuq3fURBrHqLRCup6osBJ9twQUU",
            "7Av3MR8mCMp9wu1uBaef1uSyZnbBu6qceAEyJfawPFP4",
            "7Hgp5EDpacDqat5bQBbzXPjvsBW59JCCXGDpa1ftye2Z",
            "7TKdcJLgzG55Jn1w1SoDsu8b5HQHv1EeMu3q15Zbny3X",
            "7rAYmvC4wmsU5PsVpVgv6T22zKx2kTzGVjBepkfsokqH",
            "8MD1PfodkCBFyPeifSZfwnFuXuEDbkHofXSWVoMGoC5G",
            "9Qia8LWakRm25ARA3LGgZgJJUUTn2Xvoq8fkp8x7F5Ya",
            "9k7dp4a2fRrWupvMSnBwp8GjwKs31rLdGYuWYyCqSeh2",
            "9qX9zkEHz8F97XSKSa9JjmYAvuQhxhJVLEgF4LkkavSs",
            "A4W4YNZttD7v5ova7pjoD5KJxYJniYXdgrcPdxdMoZwh",
            "ACtXeVEHg8zXhGkzRyi4GVUeaWTyMEFNV8ZFTcrjFaBa",
            "AdRmRTGQePjKxF52JZGNjpkEqADcLMYBASfpGDtn92sX",
            "B44EJEiD3GJG2vMR3DLqkahSxzdb5CkUV6Zv7ph8usjr",
            "B4ZjQ9AJDiNYVoqndxT6m6PhPxteUJCeLvZw88MyPWsU",
            "B6rjmufEZ4r7Ben475W8Mz4VyShU586ASydLmbxSBep4",
            "BeMYNZvbujnt2BYyaYHbTUzY2wvsKc7pdJ6G9792HjR8",
            "BzY7NTrg9cpa1bUdYBqnUALZxA8XqzFBXYbWXqyawF6n",
            "C9ETqZWZbMuF6gsAhGiegp2A3LASqQSm21HeXe6vpfuT",
            "CdKLWgKXSo8RdFoHezfdKWyWzVuZNAppCd1ibpbiyzTe",
            "FTtKPVbzEcaUMEiAMcB4Df9CFMpJ4zvNW36uq5FmJkiu",
            "G3fZdmSj4KE5Cn3TfRMRe8GmDPUkKsSfRZFAv7fcRw3z",
            "GiuiMyGosrEq8ChqHWSEi942xGjfy2HpZFH7vzbXHg2F",
            "Gsvv8UTa15hjbYPBH8RnNgdzeoqtsiJFzmxnstZz8vPM",
            "H7tHP5zBAh6HBCLqYekz4jpAXLXxP3CDRNwfLhK9aToU",
            "Ht3UhUzW1qQQ9Kd7VrNiwYyGTxLeyRM2H4bn6uYGNRwS",
            "J7umAcMPrj5XShTTTvHhjVFQYvNuyfF2A3VmkjEDh5jF",
            "JAt8QcSt65JFgEvty9yZewYqd7sg8RX9o84PW4Mkm375",
            "XBGPWLt7h56gZAsHyZgojcjtuvPUkcYAddHVTZDWAVP",
            "219k79Tsxx6kkXD2174kEBJ9SGsXkfDgLqLS9URwTMGc",
            "2NNmErUNF2gi42cth9534nWRhfru1TK9913JGCZWsA8x",
            "2TAvnJRVgzdmwyD6r6VVpPzkobwumR2bsrGZuvM9wT5q",
            "2h4ped9dr4cAcUqDkVzHz7Vnwwni5sJdiTXyvsVxdmmz",
            "2jAAJBnTAsDq5ThkD32XWwh9Rr5dnBBMDyTdx5WZU44n",
            "2wrNmifxANDRqACvKgLp29fkBjVWbgYjEPBKoYeqMZ1t",
            "38s9JFBhRkLJix1nWovGAjQvGKtNeyneM5Hi6igdm67P",
            "3Ss6HxYE36EMMd9h5pRpF2VaWqysc5LkYeYkTWw4i2CJ",
            "3WConF14E4exyRCrQx4txSCR5H5x2SRzyeK4pksueiLp",
            "3u2CfyvHWzcASctPLzmLzj4Hm3wAMQ8rhaSYAyKNcuci",
            "4i4vx61HyfjcKNcYDUTi6jHjNpW5apyLb4TuUkKhG7sM",
            "4rfqEHt6tW2wFpm6cHowL2NWf7gnGJ2c1eRWwCf9JDLB",
            "4tg9S9xb5M9JAqDdPLG8outfD1kcgR116F1aer4K6hLG",
            "4vsRcsSGXBtvXKEm95F9itkppum5xv8z1P7pXmcyk68r",
            "57ii8Xt6ejep5P9duENhXbxnJqJVDUFyDnJpiDirxVqY",
            "57sYCsSS94FZWFNYvwZUK2JRb5ueZZywgBHGzVQHZJ8Z",
            "5BDmYq3uSidVsWSKkB6pbJD1yeuYDB1twuEnD8ipDPW3",
            "5JAHerFFfD2MU8M8jwvXKwdBLfCgkrz1kxThFfjNtiU8",
            "5XgTKmtEX89hunVorXGcy8doCXMxf2o73bGNZnrWj81H",
            "5d49teRMfvoL6WVaNdTVnr3Ty1ZMPUo3Cfbk25DXDZPd",
            "5ppJvyhvHPxUMzLg69rnBHxsboL9XEfgpt5UTzmzip8Z",
            "5tqL5WgD7Fa6hSKSxuUrCGkyv5BxXHD4hUtyWZUxhVFd",
            "63qJgdehDp7exeBSqy7kyaqnkVozmuYeZyDpq7wwjHqn",
            "6ZSS5rHUuNkkUFk2W2phm7nWwn7szCi5B85SBHnXWDrC",
            "6arfLhQhYLt8P2ce2VqXCZ2GVM7u38ZX9m3LQhHY15nk",
            "6ehX7Tm6CdFUxZbTuVbHYFh3ZnHcdpWWppobEyEV9ovV",
            "6hGvBxGHi3vZr2G6A8byRiCVNp9tf1VGkwg3CcBRGdvq",
            "71Lghtp5dFGetx8QqzdBj4GJuY99kARieT2Qk9B2JESQ",
            "7xpEPwVXLsSUV5CMmMBxk3ShoB3e3AXdYco9HCwEhzhs",
            "7y58oqMbQQkq1KRBPNNs3iFHttRYukkFe4TYSKEgPHFW",
            "8Ax3CQfY6zcNUPwffxfzudf2Lxt3nEGSt2vdKWnwE6p6",
            "8L5r7C65Mg2AewUQzPnhFdXXzrARLdzzrPi9aCMfVG5R",
            "8N4rcGMWmCsnxFjigEeUCHj4aVfGT34bt9Gk4qvd2jFB",
            "8NYpZkAfN29Fo4XFBnTKoen93DSq2ebCqeCkiVGNcwuh",
            "8T9qHxP7HMCCrHrierNeFeJrZ9viTGTESri8Jz4MzEzb",
            "8fGXfPpvvx62HC5fTFV85ubMPZjugsso5yRvFkx1F6Ji",
            "8k9vF6AU53mUWp92Y2UmW5dZoQ25vdpD3xSQ4JrtqADm",
            "8wkuPLM1HjhYqmAWeMHCzNagPCyQncU2n53uyr2CE2ZH",
            "9UV8jS8yPbi9dSozcRXwYFA14rZWZmB2u9eiBrV9UJTQ",
            "9g4mbGGDmQ8gddzEUL1snAJ87z2EELnz7mb4Fd7t9rMi",
            "ANsYjSTVQJwyHNkktUHkJCHpVraqtVo7gvzRiMVY6f9A",
            "APezCPD1HaBcFTHuFi63YMwKFe5GFGoRtdarn7fs3i7C",
            "AQZCaaG8nBMVKQZMcfd5eDahABwfzKVrSS9pJgQ64i41",
            "AQwdCUTFiX3WJ69RyyBnRthRQYcEuRjq44mty248o8Md",
            "AfSPjzRZGJ2vFVP3p2eZp4fk5dGPLDJsSN1SKBtABTpb",
            "AiPRw6CiK5jqFqcrmRQpAQZyhUR4fnBnCKx2jQapLNJj",
            "At6cfKX99DmxBmUhwrfxjcFBu4CNfp9RETpiQBfxQY9S",
            "AuikYUkrP9bRCxPq99YpEkFCgWLS9KM2oe3sCPkTCEwr",
            "AyEE2tf4AezMxtBYXoWgoK1PwMDMsPfDahQRtZvU8BLc",
            "B5Gefd2yR3nBi4eFDtp3grmVsRq6sw4UYmGVZG6vrda3",
        ];

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

        operators_whitelist![
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
            "Cy2e827aiHG1YjPpeyhxdYLsv41GHRXGF6eXm5BhefoP",
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
            "AtrntfLuNFrqmmXsKGRiT7mYFeb5WhFvbNi5PeCwxqvK",
            "GduRLuJswRRZvC2bjFFFpeGVZyjxBv64gL3dRkT8H9WK",
            "2wGuhJ5J5qxQTyye5jkw97DG2riahzfS9TVcUrdKfYZE",
            "BftXuBxRV8sSigUk3BaYNR29d7jkGCEJ7H2pdJ5DsUs5",
            "3u9nAi7nGd9HSPNiTUoZ9Yeg2foUig4DyDjQCMZcUfuB",
            "D7yYMD3CAetZV68sfZyEYrjLEdTCy4euGHLMDDhbYqRC",
            "BgtZ5ERP1dk3pX6R3ZhLLsn9gK2712FFBJRwPLJzjf3Q",
            "GD5CtfkvLJgvSt5NhxYUvvTMP8J5njVZSjEZTgtFhByA",
            "FSWLCdQjiJcw4zFvqjdxSKtesLojygihKq2qRUcszfME",
            "86qxUGvmc5CbLrbT55W11Rxf9seSELTc2iwtZAMQCCN3",
            "5bFfsYH8mvuUkNYiJiXfRLPa66dHgFcqgtDpFsKYCGCR",
            "FLV73f4jgphZGtGqGyL88gRw145rAr9ab9HLia9WHaGa",
            "Eao4cfXw3sEVqixo18i6rVGRCbBA6HK4oth47pPkTwhF",
            "7zpspz9cf2V8sFdDMiKRWWdvmiUuX4FSgJTTVkARvos5",
            "GpobXosk2skR4SufcvN6BtAZzoCSZ2ah61uqtV3yUKvf",
            "ECjgJRuwj8KWhgaBqMwBVuvSMX3PdpbHkGmhgCBGgwmp",
            "DgyJUjow52EARe5qdQewgivgpYztUpjam1WwgxgfZSop",
            "CMXAcbx8CUKjrzGfGFXCEUFWy7pCk1DQ4nULb2sf4MfJ",
            "7rzKApzDv6unzj2qKwkAk9epQaqXPJU478oLkUpdmUDE",
            "6JnQMijtWU6Hd8Fq4TGknXkaUtsihc2LQXnyqeNvmm4V",
            "GfFStrfhsJMeKPjtUU2sbSPjZTqCe4V9e4KEpowCXQDw",
            "d3LuBwryPnEgSDX1xMV1EQbzSjz59dibvtpSknHkZWy",
            "8NHb7kThj3V4jn2FT6Qns44W3kgfAHRUp9YwiKU2iKki",
            "54WJZHA39L4UtCKTcVfY6hmLH3KfPTVrWWn9CdCozVUX",
            "2UcM2qimf1fhErp7AffzeaDAKw4uAVaL5Yf1mVmUwES1",
            "jdqK5gvjg5bCnaWTmkWhREwGWpyoz4Bf7D4zkCgq7Yr",
            "J7SCxdgTapRwUWbUqtot69kTn1YW9i3LYVpZ1CZRCktf",
            "4QzexgShewNN7vjyiGbkqgHWMmFMzQBsNy8eUFj3tueX",
            "HNBEm3pVcR2RUL2D2VGz12Fc5ABBy2sgsb6Hqw77ePGf",
            "1aPHVAsL84LFrn7AB3Gorn8hjd65FzWfp899edy62MA",
            "2gWBPL55eMW8YuNDYy6FnNhbjEDNVsXZM8xujMArkj7f",
            "9T7iuW4HqCcJWxjNcaUx2vcE67Mkpw61AWS6oYFg54oK",
            "Hc5Hndg1RXqBbGg387qUqD1eQSe4Ti6f6UWxRaFn7Csh",
            "Cso8JZBNUwZENUwKK4ggrT6dpHWY29tPJunYz7tQhGnc",
            "FBPvUsnYPvzcadGdrX9sMeqDgZeYC4knhFPUAWWGJbFV",
            "87YzMrZMgo2ujx5DCvyk1YJvwKcSvPgQ16niMSv4qBX8",
            "Fc6kL9hwZArmZggpkXUkwts2P124Uv34EbBjHCj7gdcU",
            "CWQWWPJgC5G1nJvuHKZ4R9paDca4v7kVbBw3zSBafm96",
            "2jFtMCy5biCFb2pTduaBBAFzTvQ1GsjXbP6FSaQXxYWG",
            "6aSPCHiYUpNVRQFJBT1FWvfdERA6M3VbtbeG1heg5aUJ",
            "2GR4jUxzDSnCzp5kui4Bp96xe6HDLA94E9esELvh4KdY",
            "2H694rNVMMw9RW4g2frbvjRKyu3EVfUpuTGRWCyxqmHR",
            "2o6WUWZrD2wufWD5i7d8WdkaCWkweguYcqQUQFSpuiy3",
            "3UfhW285qXsoXyBeSR7nL41f4t5omatFcXiLt5dE54R4",
            "4iYxXBHu227A57S2pWLysEnY4LiRzqStHmh3maBqi7UL",
            "4mNxgrH3my7jfiapanDV9bPTprrHzGS8v7iUY7tpiCym",
            "5MdtsuUHgkhXtRC3ChbbKbmWMsTQeCTewLVf2Ah5LrYx",
            "5V6pkMT9cCicGBvtxDJDQPUovopV4CDHnxBPxQ2CvjPt",
            "5ipjaDS6n2ZzHqU4qGQQFAcUNHgeapEDWE8r564DCpj5",
            "5swQcW8JgAxXMZqSev9mBC7Nt3PB4BiiSjjDe8PFrRmd",
            "6WUPSrVbyCHfv2iMFvVEtNzdMEePsZ7jetZLWUwmRGr8",
            "6ZcFvLtFxcaZ5PDfg25D4BZ41dvBMnhKF5eJPFxvmFZF",
            "6uCmKHoiDDG7GzmZGQuq3fURBrHqLRCup6osBJ9twQUU",
            "7Av3MR8mCMp9wu1uBaef1uSyZnbBu6qceAEyJfawPFP4",
            "7Hgp5EDpacDqat5bQBbzXPjvsBW59JCCXGDpa1ftye2Z",
            "7TKdcJLgzG55Jn1w1SoDsu8b5HQHv1EeMu3q15Zbny3X",
            "7rAYmvC4wmsU5PsVpVgv6T22zKx2kTzGVjBepkfsokqH",
            "8MD1PfodkCBFyPeifSZfwnFuXuEDbkHofXSWVoMGoC5G",
            "9Qia8LWakRm25ARA3LGgZgJJUUTn2Xvoq8fkp8x7F5Ya",
            "9k7dp4a2fRrWupvMSnBwp8GjwKs31rLdGYuWYyCqSeh2",
            "9qX9zkEHz8F97XSKSa9JjmYAvuQhxhJVLEgF4LkkavSs",
            "A4W4YNZttD7v5ova7pjoD5KJxYJniYXdgrcPdxdMoZwh",
            "ACtXeVEHg8zXhGkzRyi4GVUeaWTyMEFNV8ZFTcrjFaBa",
            "AdRmRTGQePjKxF52JZGNjpkEqADcLMYBASfpGDtn92sX",
            "B44EJEiD3GJG2vMR3DLqkahSxzdb5CkUV6Zv7ph8usjr",
            "B4ZjQ9AJDiNYVoqndxT6m6PhPxteUJCeLvZw88MyPWsU",
            "B6rjmufEZ4r7Ben475W8Mz4VyShU586ASydLmbxSBep4",
            "BeMYNZvbujnt2BYyaYHbTUzY2wvsKc7pdJ6G9792HjR8",
            "BzY7NTrg9cpa1bUdYBqnUALZxA8XqzFBXYbWXqyawF6n",
            "C9ETqZWZbMuF6gsAhGiegp2A3LASqQSm21HeXe6vpfuT",
            "CdKLWgKXSo8RdFoHezfdKWyWzVuZNAppCd1ibpbiyzTe",
            "FTtKPVbzEcaUMEiAMcB4Df9CFMpJ4zvNW36uq5FmJkiu",
            "G3fZdmSj4KE5Cn3TfRMRe8GmDPUkKsSfRZFAv7fcRw3z",
            "GiuiMyGosrEq8ChqHWSEi942xGjfy2HpZFH7vzbXHg2F",
            "Gsvv8UTa15hjbYPBH8RnNgdzeoqtsiJFzmxnstZz8vPM",
            "H7tHP5zBAh6HBCLqYekz4jpAXLXxP3CDRNwfLhK9aToU",
            "Ht3UhUzW1qQQ9Kd7VrNiwYyGTxLeyRM2H4bn6uYGNRwS",
            "J7umAcMPrj5XShTTTvHhjVFQYvNuyfF2A3VmkjEDh5jF",
            "JAt8QcSt65JFgEvty9yZewYqd7sg8RX9o84PW4Mkm375",
            "XBGPWLt7h56gZAsHyZgojcjtuvPUkcYAddHVTZDWAVP",
            "219k79Tsxx6kkXD2174kEBJ9SGsXkfDgLqLS9URwTMGc",
            "2NNmErUNF2gi42cth9534nWRhfru1TK9913JGCZWsA8x",
            "2TAvnJRVgzdmwyD6r6VVpPzkobwumR2bsrGZuvM9wT5q",
            "2h4ped9dr4cAcUqDkVzHz7Vnwwni5sJdiTXyvsVxdmmz",
            "2jAAJBnTAsDq5ThkD32XWwh9Rr5dnBBMDyTdx5WZU44n",
            "2wrNmifxANDRqACvKgLp29fkBjVWbgYjEPBKoYeqMZ1t",
            "38s9JFBhRkLJix1nWovGAjQvGKtNeyneM5Hi6igdm67P",
            "3Ss6HxYE36EMMd9h5pRpF2VaWqysc5LkYeYkTWw4i2CJ",
            "3WConF14E4exyRCrQx4txSCR5H5x2SRzyeK4pksueiLp",
            "3u2CfyvHWzcASctPLzmLzj4Hm3wAMQ8rhaSYAyKNcuci",
            "4i4vx61HyfjcKNcYDUTi6jHjNpW5apyLb4TuUkKhG7sM",
            "4rfqEHt6tW2wFpm6cHowL2NWf7gnGJ2c1eRWwCf9JDLB",
            "4tg9S9xb5M9JAqDdPLG8outfD1kcgR116F1aer4K6hLG",
            "4vsRcsSGXBtvXKEm95F9itkppum5xv8z1P7pXmcyk68r",
            "57ii8Xt6ejep5P9duENhXbxnJqJVDUFyDnJpiDirxVqY",
            "57sYCsSS94FZWFNYvwZUK2JRb5ueZZywgBHGzVQHZJ8Z",
            "5BDmYq3uSidVsWSKkB6pbJD1yeuYDB1twuEnD8ipDPW3",
            "5JAHerFFfD2MU8M8jwvXKwdBLfCgkrz1kxThFfjNtiU8",
            "5XgTKmtEX89hunVorXGcy8doCXMxf2o73bGNZnrWj81H",
            "5d49teRMfvoL6WVaNdTVnr3Ty1ZMPUo3Cfbk25DXDZPd",
            "5ppJvyhvHPxUMzLg69rnBHxsboL9XEfgpt5UTzmzip8Z",
            "5tqL5WgD7Fa6hSKSxuUrCGkyv5BxXHD4hUtyWZUxhVFd",
            "63qJgdehDp7exeBSqy7kyaqnkVozmuYeZyDpq7wwjHqn",
            "6ZSS5rHUuNkkUFk2W2phm7nWwn7szCi5B85SBHnXWDrC",
            "6arfLhQhYLt8P2ce2VqXCZ2GVM7u38ZX9m3LQhHY15nk",
            "6ehX7Tm6CdFUxZbTuVbHYFh3ZnHcdpWWppobEyEV9ovV",
            "6hGvBxGHi3vZr2G6A8byRiCVNp9tf1VGkwg3CcBRGdvq",
            "71Lghtp5dFGetx8QqzdBj4GJuY99kARieT2Qk9B2JESQ",
            "7xpEPwVXLsSUV5CMmMBxk3ShoB3e3AXdYco9HCwEhzhs",
            "7y58oqMbQQkq1KRBPNNs3iFHttRYukkFe4TYSKEgPHFW",
            "8Ax3CQfY6zcNUPwffxfzudf2Lxt3nEGSt2vdKWnwE6p6",
            "8L5r7C65Mg2AewUQzPnhFdXXzrARLdzzrPi9aCMfVG5R",
            "8N4rcGMWmCsnxFjigEeUCHj4aVfGT34bt9Gk4qvd2jFB",
            "8NYpZkAfN29Fo4XFBnTKoen93DSq2ebCqeCkiVGNcwuh",
            "8T9qHxP7HMCCrHrierNeFeJrZ9viTGTESri8Jz4MzEzb",
            "8fGXfPpvvx62HC5fTFV85ubMPZjugsso5yRvFkx1F6Ji",
            "8k9vF6AU53mUWp92Y2UmW5dZoQ25vdpD3xSQ4JrtqADm",
            "8wkuPLM1HjhYqmAWeMHCzNagPCyQncU2n53uyr2CE2ZH",
            "9UV8jS8yPbi9dSozcRXwYFA14rZWZmB2u9eiBrV9UJTQ",
            "9g4mbGGDmQ8gddzEUL1snAJ87z2EELnz7mb4Fd7t9rMi",
            "ANsYjSTVQJwyHNkktUHkJCHpVraqtVo7gvzRiMVY6f9A",
            "APezCPD1HaBcFTHuFi63YMwKFe5GFGoRtdarn7fs3i7C",
            "AQZCaaG8nBMVKQZMcfd5eDahABwfzKVrSS9pJgQ64i41",
            "AQwdCUTFiX3WJ69RyyBnRthRQYcEuRjq44mty248o8Md",
            "AfSPjzRZGJ2vFVP3p2eZp4fk5dGPLDJsSN1SKBtABTpb",
            "AiPRw6CiK5jqFqcrmRQpAQZyhUR4fnBnCKx2jQapLNJj",
            "At6cfKX99DmxBmUhwrfxjcFBu4CNfp9RETpiQBfxQY9S",
            "AuikYUkrP9bRCxPq99YpEkFCgWLS9KM2oe3sCPkTCEwr",
            "AyEE2tf4AezMxtBYXoWgoK1PwMDMsPfDahQRtZvU8BLc",
            "B5Gefd2yR3nBi4eFDtp3grmVsRq6sw4UYmGVZG6vrda3",
        ];

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

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "95tQS9NwHyboQm31za2FyNdxR8NVgqripwRUjZD97nrz");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "EqhCRgbZqCaXs6S8T2U2TJHkAffuNS99ot3ueFeUXJRF");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
        
    } else if #[cfg(feature = "govertest")] {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 111;

        operators_whitelist![
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
        ];

        /// Token Mint ID
        pub mod token_mint {
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, "EjLGfD8mpxKLwGDi8AiTisAbGtWWM2L3htkJ6MpvS8Hk");
            /// Ethereum account version
            pub const DECIMALS: u8 = 9;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, "B2m2PGZQuZzaVMkeH8fLR8EbefiEy64ybCxVuzhx6RD1");
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, "D73ziEn1qS4egcMfADTZJnnn5XCENdcrDDcwAnSEvqGX");
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, "0");
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, "0");
        }
        
    } else {

        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = 111;

        operators_whitelist![
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
        ];
    
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
pub const REQUEST_UNITS_ADDITIONAL_FEE: u64 = 0;
/// Gas limit multiplier for transactions without chain id
pub const GAS_LIMIT_MULTIPLIER_NO_CHAINID: u32 = 1000;
/// Amount of storage entries stored in the contract account
pub const STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT: u32 = 64;
/// Minimum number of EVM steps for iterative transaction
pub const EVM_STEPS_MIN: u64 = 500;
/// Maximum number of EVM steps in a last iteration
pub const EVM_STEPS_LAST_ITERATION_MAX: u64 = 0;

cfg_if! {
    if #[cfg(feature = "emergency")] {
        neon_elf_param!( NEON_STATUS_NAME, "EMERGENCY");
    } else {
        neon_elf_param!( NEON_STATUS_NAME, "WORK");
    }
}


neon_elf_param!( NEON_PKG_VERSION           , env!("CARGO_PKG_VERSION"));
neon_elf_param!( NEON_REVISION              , env!("NEON_REVISION"));
neon_elf_param!( NEON_SEED_VERSION          , formatcp!("{:?}", ACCOUNT_SEED_VERSION));
neon_elf_param!( NEON_TOKEN_MINT_DECIMALS   , formatcp!("{:?}", token_mint::DECIMALS));
neon_elf_param!( NEON_PAYMENT_TO_TREASURE   , formatcp!("{:?}", PAYMENT_TO_TREASURE));
neon_elf_param!( NEON_PAYMENT_TO_DEPOSIT    , formatcp!("{:?}", PAYMENT_TO_DEPOSIT));
neon_elf_param!( NEON_CHAIN_ID              , formatcp!("{:?}", CHAIN_ID));
neon_elf_param!( NEON_POOL_SEED             , formatcp!("{}",   TREASURY_POOL_SEED));
neon_elf_param!( NEON_POOL_COUNT            , formatcp!("{:?}", TREASURY_POOL_COUNT));
neon_elf_param!( NEON_HOLDER_MSG_SIZE       , formatcp!("{:?}", HOLDER_MSG_SIZE));
neon_elf_param!( NEON_COMPUTE_UNITS         , formatcp!("{:?}", COMPUTE_BUDGET_UNITS));
neon_elf_param!( NEON_HEAP_FRAME            , formatcp!("{:?}", COMPUTE_BUDGET_HEAP_FRAME));
neon_elf_param!( NEON_ADDITIONAL_FEE        , formatcp!("{:?}", REQUEST_UNITS_ADDITIONAL_FEE));
neon_elf_param!( NEON_GAS_LIMIT_MULTIPLIER_NO_CHAINID, formatcp!("{:?}", GAS_LIMIT_MULTIPLIER_NO_CHAINID));
neon_elf_param!( NEON_STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT, formatcp!("{:?}", STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT));
neon_elf_param!( NEON_EVM_STEPS_MIN, formatcp!("{:?}", EVM_STEPS_MIN));

/// Chain ID
#[must_use]
pub fn chain_id() -> U256 {
    U256::from(CHAIN_ID)
}

