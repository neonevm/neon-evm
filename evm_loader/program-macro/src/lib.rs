mod config_parser;

use std::env;
use std::path::PathBuf;

use config_parser::{
    AccountWhitelists, CollateralPoolBase, CommonConfig, NetSpecificConfig, TokenMint,
};
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Expr, Ident, LitStr, Result, Token};

use quote::quote;

extern crate proc_macro;

struct OperatorsWhitelistInput {
    list: Punctuated<LitStr, Token![,]>,
}

impl Parse for OperatorsWhitelistInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let list = Punctuated::parse_terminated(input)?;
        Ok(Self { list })
    }
}

#[proc_macro]
pub fn operators_whitelist(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as OperatorsWhitelistInput);

    let mut operators: Vec<Vec<u8>> = input
        .list
        .iter()
        .map(LitStr::value)
        .map(|key| bs58::decode(key).into_vec().unwrap())
        .collect();

    operators.sort_unstable();

    let len = operators.len();

    quote! {
        pub static AUTHORIZED_OPERATOR_LIST: [::solana_program::pubkey::Pubkey; #len] = [
            #(::solana_program::pubkey::Pubkey::new_from_array([#((#operators),)*]),)*
        ];
    }
    .into()
}

struct ElfParamInput {
    name: Ident,
    _separator: Token![,],
    value: Expr,
}

impl Parse for ElfParamInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            _separator: input.parse()?,
            value: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn neon_elf_param(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as ElfParamInput);

    let name = input.name;
    let value = input.value;

    quote! {
        #[no_mangle]
        #[used]
        #[doc(hidden)]
        pub static #name: [u8; #value.len()] = {
            #[allow(clippy::string_lit_as_bytes)]
            let bytes: &[u8] = #value.as_bytes();

            let mut array = [0; #value.len()];
            let mut i = 0;
            while i < #value.len() {
                array[i] = bytes[i];
                i += 1;
            }
            array
        };
    }
    .into()
}

struct ElfParamIdInput {
    name: Ident,
    _separator: Token![,],
    value: LitStr,
}

impl Parse for ElfParamIdInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            name: input.parse()?,
            _separator: input.parse()?,
            value: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn declare_param_id(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as ElfParamIdInput);

    let name = input.name;

    let value = input.value.value();
    let value_bytes = value.as_bytes();

    let len = value.len();

    quote! {
        ::solana_program::declare_id!(#value);

        #[no_mangle]
        #[used]
        #[doc(hidden)]
        pub static #name: [u8; #len] = [
            #((#value_bytes),)*
        ];
    }
    .into()
}

#[proc_macro]
pub fn net_specific_config_parser(tokens: TokenStream) -> TokenStream {
    let file_relative_path = parse_macro_input!(tokens as LitStr);
    let mut file_path: PathBuf = PathBuf::new();
    file_path.push(env::var("CARGO_MANIFEST_DIR").unwrap());
    file_path.push(file_relative_path.value());
    let file_contents = std::fs::read(&file_path)
        .expect(&format!("{} should be a valid path", file_path.display()));

    let NetSpecificConfig {
        chain_id,
        operators_whitelist,
        token_mint: TokenMint {
            neon_token_mint,
            decimals,
        },
        collateral_pool_base:
            CollateralPoolBase {
                neon_pool_base,
                prefix,
                main_balance_seed,
                neon_pool_count,
            },
        account_whitelists:
            AccountWhitelists {
                neon_permission_allowance_token,
                neon_permission_denial_token,
                neon_minimal_client_allowance_balance,
                neon_minimal_contract_allowance_balance,
            },
    } = toml::from_slice(&file_contents).expect("File should parse to a Config");

    quote! {
        /// Supported CHAIN_ID value for transactions
        pub const CHAIN_ID: u64 = #chain_id;

        operators_whitelist![#(#operators_whitelist),*];

        /// Token Mint ID
        pub mod token_mint {
            use super::declare_param_id;

            declare_param_id!(NEON_TOKEN_MINT, #neon_token_mint);
            /// Ethereum account version
            pub const DECIMALS: u8 = #decimals;

            /// Number of base 10 digits to the right of the decimal place
            #[must_use]
            pub const fn decimals() -> u8 { DECIMALS }

        }

        /// Collateral pool base address
        pub mod collateral_pool_base {
            use super::declare_param_id;

            declare_param_id!(NEON_POOL_BASE, #neon_pool_base);

            /// `COLLATERAL_SEED_PREFIX`
            pub const PREFIX: &str = #prefix;

            /// Treasury pool main balance seed
            pub const MAIN_BALANCE_SEED: &str = #main_balance_seed;

            /// Count of balances in collaterail pool
            pub const NEON_POOL_COUNT: u32 = #neon_pool_count;
        }

        /// Account whitelists: Permission tokens
        pub mod account_whitelists {
            use super::neon_elf_param;

            neon_elf_param!(NEON_PERMISSION_ALLOWANCE_TOKEN, #neon_permission_allowance_token);
            neon_elf_param!(NEON_PERMISSION_DENIAL_TOKEN, #neon_permission_denial_token);
            neon_elf_param!(NEON_MINIMAL_CLIENT_ALLOWANCE_BALANCE, #neon_minimal_client_allowance_balance);
            neon_elf_param!(NEON_MINIMAL_CONTRACT_ALLOWANCE_BALANCE, #neon_minimal_contract_allowance_balance);
        }
    }
    .into()
}

#[proc_macro]
pub fn common_config_parser(tokens: TokenStream) -> TokenStream {
    let file_relative_path = parse_macro_input!(tokens as LitStr);
    let mut file_path: PathBuf = PathBuf::new();
    file_path.push(env::var("CARGO_MANIFEST_DIR").unwrap());
    file_path.push(file_relative_path.value());
    let file_contents = std::fs::read(&file_path)
        .expect(&format!("{} should be a valid path", file_path.display()));

    let CommonConfig {
        payment_to_treasure,
        payment_to_deposit,
        operator_priority_slots,
        holder_msg_size,
        compute_budget_units,
        compute_budget_heap_frame,
        request_units_additional_fee,
        gas_limit_multiplier_no_chainid,
        storage_entries_in_contract_account,
        evm_steps_min,
        evm_steps_last_iteration_max,
    } = toml::from_slice(&file_contents).expect("File should parse to a Config");
    quote! {
        /// `PAYMENT_TO_COLLATERAL_POOL`
        pub const PAYMENT_TO_TREASURE: u64 = #payment_to_treasure;
        /// `PAYMENT_TO_DEPOSIT`
        pub const PAYMENT_TO_DEPOSIT: u64 = #payment_to_deposit;
        /// `OPERATOR_PRIORITY_SLOTS`
        pub const OPERATOR_PRIORITY_SLOTS: u64 = #operator_priority_slots;
        /// `the message size that is used to holder-account filling`
        pub const HOLDER_MSG_SIZE: u64 = #holder_msg_size;
        /// `OPERATOR_PRIORITY_SLOTS`
        pub const COMPUTE_BUDGET_UNITS: u32 = #compute_budget_units;
        /// `OPERATOR_PRIORITY_SLOTS`
        pub const COMPUTE_BUDGET_HEAP_FRAME: u32 = #compute_budget_heap_frame;
        /// Additional fee for `request units` instruction
        pub const REQUEST_UNITS_ADDITIONAL_FEE: u64 = #request_units_additional_fee;
        /// Gas limit multiplier for transactions without chain id
        pub const GAS_LIMIT_MULTIPLIER_NO_CHAINID: u32 = #gas_limit_multiplier_no_chainid;
        /// Amount of storage entries stored in the contract account
        pub const STORAGE_ENTRIES_IN_CONTRACT_ACCOUNT: u32 = #storage_entries_in_contract_account;
        /// Minimum number of EVM steps for iterative transaction
        pub const EVM_STEPS_MIN: u64 = #evm_steps_min;
        /// Maximum number of EVM steps in a last iteration
        pub const EVM_STEPS_LAST_ITERATION_MAX: u64 = #evm_steps_last_iteration_max;
    }
    .into()
}
