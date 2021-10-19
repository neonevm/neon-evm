extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{ Span };
use quote::{quote, ToTokens};
use std::convert::TryFrom;
use syn::{
    bracketed,
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    punctuated::Punctuated,
    token::Bracket,
    Ident, LitByte, LitStr, Token,
};


fn parse_pubkey(
    id_literal: &LitStr,
    pubkey_type: &proc_macro2::TokenStream,
) -> Result<proc_macro2::TokenStream> {
    let id_vec = bs58::decode(id_literal.value())
        .into_vec()
        .map_err(|_| syn::Error::new_spanned(&id_literal, "failed to decode base58 string"))?;
    let id_array = <[u8; 32]>::try_from(<&[u8]>::clone(&&id_vec[..])).map_err(|_| {
        syn::Error::new_spanned(
            &id_literal,
            format!("pubkey array is not 32 bytes long: len={}", id_vec.len()),
        )
    })?;
    let bytes = id_array.iter().map(|b| LitByte::new(*b, Span::call_site()));
    Ok(quote! {
        #pubkey_type::new_from_array(
            [#(#bytes,)*]
        )
    })
}

struct PubkeyArray {
    name: Ident,
    num: usize,
    pubkeys: proc_macro2::TokenStream,
}

impl Parse for PubkeyArray {
    fn parse(input: ParseStream) -> Result<Self> {
        let pubkey_type = quote! {
            ::solana_program::pubkey::Pubkey
        };

        let name = input.parse()?;
        let _comma: Token![,] = input.parse()?;
        let (num, pubkeys) = if input.peek(syn::LitStr) {
            let id_literal: LitStr = input.parse()?;
            (1, parse_pubkey(&id_literal, &pubkey_type)?)
        } else if input.peek(Bracket) {
            let pubkey_strings;
            bracketed!(pubkey_strings in input);
            let punctuated: Punctuated<LitStr, Token![,]> =
                Punctuated::parse_terminated(&pubkey_strings)?;
            let mut pubkeys: Punctuated<proc_macro2::TokenStream, Token![,]> = Punctuated::new();
            for string in punctuated.iter() {
                pubkeys.push(parse_pubkey(string, &pubkey_type)?);
            }
            (pubkeys.len(), quote! {#pubkeys})
        } else {
            let stream: proc_macro2::TokenStream = input.parse()?;
            return Err(syn::Error::new_spanned(stream, "unexpected token"));
        };

        Ok(PubkeyArray {
            name,
            num,
            pubkeys,
        })
    }
}

impl ToTokens for PubkeyArray {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {

        let PubkeyArray {
            name,
            num,
            pubkeys,
        } = self;

        let pubkey_type = quote! {
            ::solana_program::pubkey::Pubkey
        };
        tokens.extend(quote! {
            /// The const Pubkey array
            pub const #name: [#pubkey_type; #num] = [#pubkeys];
        });
    }
}

#[proc_macro]
pub fn pubkey_array(input: TokenStream) -> TokenStream {
    let pubkeys = parse_macro_input!(input as PubkeyArray);
    TokenStream::from(quote! {#pubkeys})
}
