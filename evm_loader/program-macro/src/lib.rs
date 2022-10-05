use proc_macro::TokenStream;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Result, Token, LitStr, Ident, Expr};
use syn::parse::{Parse, ParseStream};

use quote::{quote};

extern crate proc_macro;

struct OperatorsWhitelistInput {
    list: Punctuated<LitStr, Token![,]>,
}

impl Parse for OperatorsWhitelistInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let list = Punctuated::parse_terminated(&input)?;
        Ok(Self{list})
    }
}

#[proc_macro]
pub fn operators_whitelist(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as OperatorsWhitelistInput);

    let mut operators: Vec<Vec<u8>> = input.list.iter()
        .map(LitStr::value)
        .map(|key| bs58::decode(key).into_vec().unwrap())
        .collect();

    operators.sort_unstable();

    let len = operators.len();

    quote! {
        pub static AUTHORIZED_OPERATOR_LIST: [::solana_program::pubkey::Pubkey; #len] = [
            #(::solana_program::pubkey::Pubkey::new_from_array([#((#operators),)*]),)*
        ];
    }.into()
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
            value: input.parse()?
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
    }.into()
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
            value: input.parse()?
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
    }.into()
}