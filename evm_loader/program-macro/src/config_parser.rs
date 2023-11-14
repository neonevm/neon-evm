use std::path::PathBuf;

use itertools::Itertools;
use proc_macro2::Literal;
use quote::quote;
use serde::Deserialize;
use syn::{
    parse::{Parse, ParseStream},
    parse_str, Ident, Lit, LitBool, LitFloat, LitInt, LitStr, Type,
};
use toml::Table;

#[derive(Deserialize)]
pub struct NetSpecificConfig {
    pub program_id: String,
    pub operators_whitelist: Vec<String>,
    pub neon_chain_id: u64,
    pub neon_token_mint: String,
    pub chains: Vec<Chain>,
}

impl Parse for NetSpecificConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let file_relative_path = input.parse::<LitStr>()?.value();

        let file_path = PathBuf::from_iter([manifest_dir, file_relative_path]);

        let file_contents = std::fs::read_to_string(file_path).unwrap();

        let root = file_contents.parse::<Table>().unwrap();

        let program_id = root["program_id"].as_str().unwrap().to_string();
        let operators_whitelist = root["operators_whitelist"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();

        let chains = root["chain"]
            .as_table()
            .unwrap()
            .iter()
            .map(|(name, table)| {
                let table = table.as_table().unwrap();
                Chain {
                    id: table["id"].as_integer().unwrap().try_into().unwrap(),
                    name: name.clone(),
                    token: table["token"].as_str().unwrap().to_string(),
                }
            })
            .collect::<Vec<_>>();

        let (neon_chain_id, neon_token_mint) = chains
            .iter()
            .find_map(|c| {
                if c.name == "neon" {
                    Some((c.id, c.token.clone()))
                } else {
                    None
                }
            })
            .unwrap();

        Ok(Self {
            program_id,
            operators_whitelist,
            neon_chain_id,
            neon_token_mint,
            chains,
        })
    }
}

#[derive(Deserialize, Debug)]
pub struct Chain {
    pub id: u64,
    pub name: String,
    pub token: String,
}

pub struct CommonVariable {
    pub name: Ident,
    pub r#type: Type,
    pub value: Lit,
}

pub struct CommonConfig {
    pub variables: Vec<CommonVariable>,
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
        let file_contents = std::fs::read_to_string(&file_path).map_err(|_| {
            syn::Error::new(
                input.span(),
                format!("{} should be a valid path", file_path.display()),
            )
        })?;
        let config = file_contents
            .parse::<Table>()
            .map_err(|e| syn::Error::new(input.span(), e.to_string()))?;

        let variables: Vec<_> = config
            .into_iter()
            .map(move |(name, value)| {
                let name = name.to_uppercase();
                let name: Ident = parse_str(&name)?;

                match value {
                    toml::Value::Float(v) => Ok(CommonVariable {
                        name,
                        r#type: Type::Verbatim(quote!(f64)),
                        value: Lit::new(Literal::f64_unsuffixed(v)),
                    }),
                    toml::Value::Integer(v) => Ok(CommonVariable {
                        name,
                        r#type: Type::Verbatim(quote!(u64)),
                        value: Lit::new(Literal::i64_unsuffixed(v)),
                    }),
                    toml::Value::String(v) => Ok(CommonVariable {
                        name,
                        r#type: Type::Verbatim(quote!(&str)),
                        value: Lit::Str(LitStr::new(&v, input.span())),
                    }),
                    toml::Value::Boolean(v) => Ok(CommonVariable {
                        name,
                        r#type: Type::Verbatim(quote!(bool)),
                        value: Lit::Bool(LitBool::new(v, input.span())),
                    }),
                    toml::Value::Array(ref array) => match (array.get(0), array.get(1)) {
                        (Some(toml::Value::Integer(v)), Some(toml::Value::String(t))) => {
                            let s = v.to_string();
                            let v: LitInt = parse_str(&s)?;
                            let t: Type = parse_str(t)?;
                            Ok(CommonVariable {
                                name,
                                r#type: t,
                                value: Lit::Int(v),
                            })
                        }
                        (Some(toml::Value::Float(v)), Some(toml::Value::String(t))) => {
                            let s = v.to_string();
                            let v: LitFloat = parse_str(&s)?;
                            let t: Type = parse_str(t)?;
                            Ok(CommonVariable {
                                name,
                                r#type: t,
                                value: Lit::Float(v),
                            })
                        }
                        _ => Err(syn::Error::new(
                            input.span(),
                            format!("Unsupported TOML value {value:?}"),
                        )),
                    },
                    _ => Err(syn::Error::new(
                        input.span(),
                        format!("Unsupported TOML value {value:?}"),
                    )),
                }
            })
            .try_collect()?;

        Ok(Self { variables })
    }
}
