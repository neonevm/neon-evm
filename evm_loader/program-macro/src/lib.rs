#![deny(warnings)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]

mod config_parser;

use std::collections::BTreeMap;

use config_parser::{CommonConfig, NetSpecificConfig};
use proc_macro::TokenStream;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Expr, Ident, LitStr, Result, Token};

use quote::quote;

extern crate proc_macro;

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

/// # Panics
/// Panic at compile time if config file is not correct
#[proc_macro]
pub fn net_specific_config_parser(tokens: TokenStream) -> TokenStream {
    let NetSpecificConfig {
        program_id,
        neon_chain_id,
        neon_token_mint,
        operators_whitelist,
        mut chains,
    } = parse_macro_input!(tokens as NetSpecificConfig);

    let mut operators: Vec<Vec<u8>> = operators_whitelist
        .iter()
        .map(|key| bs58::decode(key).into_vec().unwrap())
        .collect();

    operators.sort_unstable();
    let operators_len = operators.len();

    chains.sort_unstable_by_key(|c| c.id);
    let chains_len = chains.len();

    let chain_ids = chains.iter().map(|c| c.id).collect::<Vec<_>>();
    let chain_names = chains.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
    let chain_tokens = chains
        .iter()
        .map(|c| bs58::decode(&c.token).into_vec().unwrap())
        .collect::<Vec<_>>();

    let neon_chain_id_str = neon_chain_id.to_string();

    quote! {
        pub const PROGRAM_ID: solana_program::pubkey::Pubkey = solana_program::pubkey!(#program_id);
        pub const DEFAULT_CHAIN_ID: u64 = #neon_chain_id;

        neon_elf_param!(NEON_CHAIN_ID, #neon_chain_id_str);
        neon_elf_param!(NEON_TOKEN_MINT, #neon_token_mint);

        pub static AUTHORIZED_OPERATOR_LIST: [::solana_program::pubkey::Pubkey; #operators_len] = [
            #(::solana_program::pubkey::Pubkey::new_from_array([#((#operators),)*]),)*
        ];

        pub static CHAIN_ID_LIST: [(u64, &str, ::solana_program::pubkey::Pubkey); #chains_len] = [
            #( (#chain_ids, #chain_names, ::solana_program::pubkey::Pubkey::new_from_array([#(#chain_tokens),*])) ),*
        ];
    }
    .into()
}

#[proc_macro]
pub fn common_config_parser(tokens: TokenStream) -> TokenStream {
    let config = parse_macro_input!(tokens as CommonConfig);

    let mut variables = BTreeMap::new();
    let mut tokens = Vec::<proc_macro2::TokenStream>::new();

    for v in config.variables {
        let t = v.r#type;
        let name = v.name;
        let value = v.value;

        let elf_name_string = "NEON_".to_string() + &name.to_string();
        let elf_name = Ident::new(&elf_name_string, name.span());
        let elf_value = match &value {
            syn::Lit::Str(s) => s.clone(),
            syn::Lit::Int(i) => LitStr::new(&i.to_string(), i.span()),
            syn::Lit::Float(f) => LitStr::new(&f.to_string(), f.span()),
            syn::Lit::Bool(b) => LitStr::new(&b.value().to_string(), b.span()),
            _ => unreachable!(),
        };

        tokens.push(quote! {
            pub const #name: #t = #value;
            neon_elf_param!(#elf_name, #elf_value);
        });

        variables.insert(elf_name_string, elf_value);
    }

    let variables_len = variables.len();
    let variable_names = variables.keys();
    let variable_values = variables.values();

    quote! {
        #(#tokens)*

        pub static PARAMETERS: [(&str, &str); #variables_len] = [
            #( (#variable_names, #variable_values) ),*
        ];
    }
    .into()
}
