use std::{collections::HashMap, path::PathBuf};

use itertools::Itertools;
use proc_macro::TokenStream;
use quote::quote;
use serde::Deserialize;
use syn::{
    parse::{Parse, ParseStream},
    parse_str, Expr, Ident, LitFloat, LitInt, LitStr, Type,
};

#[derive(Deserialize)]
pub struct NetSpecificConfig {
    pub chain_id: u64,
    pub operators_whitelist: Vec<String>,
    pub token_mint: TokenMint,
    pub collateral_pool_base: CollateralPoolBase,
    pub account_whitelists: AccountWhitelists,
}

impl Parse for NetSpecificConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let file_relative_path: LitStr = input.parse()?;
        let mut file_path = PathBuf::new();
        file_path.push(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
            syn::Error::new(
                input.span(),
                "This proc macro should be called from a Cargo project",
            )
        })?);
        file_path.push(file_relative_path.value());
        let file_contents = std::fs::read(&file_path).map_err(|_| {
            syn::Error::new(
                input.span(),
                &format!("{} should be a valid path", file_path.display()),
            )
        })?;
        toml::from_slice(&file_contents).map_err(|e| syn::Error::new(input.span(), &e.to_string()))
    }
}

#[derive(Deserialize)]
pub struct TokenMint {
    pub neon_token_mint: String,
    pub decimals: u8,
}

#[derive(Deserialize)]
pub struct CollateralPoolBase {
    pub neon_pool_base: String,
    pub prefix: String,
    pub main_balance_seed: String,
    pub neon_pool_count: u32,
}

#[derive(Deserialize)]
pub struct AccountWhitelists {
    pub neon_permission_allowance_token: String,
    pub neon_permission_denial_token: String,
    pub neon_minimal_client_allowance_balance: String,
    pub neon_minimal_contract_allowance_balance: String,
}

pub struct CommonConfig {
    pub token_stream: TokenStream,
}

impl Parse for CommonConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let file_relative_path: LitStr = input.parse()?;
        let mut file_path = PathBuf::new();
        file_path.push(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
            syn::Error::new(
                input.span(),
                "This proc macro should be called from a Cargo project",
            )
        })?);
        file_path.push(file_relative_path.value());
        let file_contents = std::fs::read(&file_path).map_err(|_| {
            syn::Error::new(
                input.span(),
                &format!("{} should be a valid path", file_path.display()),
            )
        })?;
        let config: HashMap<String, HashMap<String, toml::Value>> =
            toml::from_slice(&file_contents)
                .map_err(|e| syn::Error::new(input.span(), &e.to_string()))?;
        let variables: Vec<_> = config
            .into_iter()
            .flat_map(|(r#type, variables)| {
                variables
                    .into_iter()
                    .map(move |(name, value)| {
                        let uppercased_name = name.to_uppercase();
                        let ident_name: Ident = parse_str(&uppercased_name)?;
                        let ident_type: Type = parse_str(&r#type)?;
                        let neon_ident_name: Ident =
                            parse_str(&format!("NEON_{}", uppercased_name))?;
                        match value {
                            toml::Value::Float(v) => {
                                let v: LitFloat = parse_str(&v.to_string())?;
                                Ok(quote! {
                                    pub const #ident_name: #ident_type = #v;
                                    neon_elf_param!(#neon_ident_name, formatcp!("{:?}", #ident_name));
                                })
                            }
                            toml::Value::Integer(v) => {
                                let v: LitInt = parse_str(&v.to_string())?;
                                Ok(quote! {
                                    pub const #ident_name: #ident_type = #v;
                                    neon_elf_param!(#neon_ident_name, formatcp!("{:?}", #ident_name));
                                })
                            }
                            toml::Value::String(v) => {
                                Ok(quote! {
                                    pub const #ident_name: #ident_type = #v;
                                    neon_elf_param!(#neon_ident_name, formatcp!("{:?}", #ident_name));
                                })
                            }
                            toml::Value::Boolean(v) => {
                                Ok(quote! {
                                    pub const #ident_name: #ident_type = #v;
                                    neon_elf_param!(#neon_ident_name, formatcp!("{:?}", #ident_name));
                                })
                            }
                            _ => {
                                Err(syn::Error::new(
                                    input.span(),
                                    &format!("Unsupported TOML value {:?}", value),
                                ))
                            }
                        }
                    })
                    .flatten_ok()
            })
            .try_collect()?;

        Ok(Self {
            token_stream: quote! {#(#variables)*}.into(),
        })
    }
}

#[derive(Deserialize)]
struct InternalElfParams {
    env: HashMap<String, String>,
    extra: HashMap<String, String>,
}

pub struct ElfParams {
    pub token_stream: TokenStream,
}

impl Parse for ElfParams {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let file_relative_path: LitStr = input.parse()?;
        let mut file_path = PathBuf::new();
        file_path.push(std::env::var("CARGO_MANIFEST_DIR").map_err(|_| {
            syn::Error::new(
                input.span(),
                "This proc macro should be called from a Cargo project",
            )
        })?);
        file_path.push(file_relative_path.value());
        let file_contents = std::fs::read(&file_path).map_err(|_| {
            syn::Error::new(
                input.span(),
                &format!("{} should be a valid path", file_path.display()),
            )
        })?;
        let InternalElfParams { env, extra } = toml::from_slice(&file_contents)
            .map_err(|e| syn::Error::new(input.span(), &e.to_string()))?;
        let env_tokens = env
            .into_iter()
            .map(|(name, env_name)| {
                let name_ident: Ident = parse_str(&name.to_uppercase())?;
                Ok(quote! { neon_elf_param!(#name_ident, env!(#env_name)); })
            })
            .flatten_ok()
            .try_collect::<_, Vec<_>, syn::Error>()?;

        let extra_tokens = extra
            .into_iter()
            .map(|(name, value)| {
                let name_ident: Ident = parse_str(&name.to_uppercase())?;
                let value_expr: Expr = parse_str(&value)?;
                Ok(quote! { neon_elf_param!(#name_ident, formatcp!("{:?}", #value_expr)); })
            })
            .flatten_ok()
            .try_collect::<_, Vec<_>, syn::Error>()?;

        Ok(ElfParams {
            token_stream: quote! {
                #(#env_tokens)*
                #(#extra_tokens)*
            }
            .into(),
        })
    }
}
