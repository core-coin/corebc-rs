//! Helper functions for deriving `EthAbiType`

use corebc_core::macros::corebc_core_crate;
use quote::quote;
use syn::DeriveInput;

/// Generates the `AbiEncode` + `AbiDecode` implementation
pub fn derive_codec_impl(input: &DeriveInput) -> proc_macro2::TokenStream {
    let name = &input.ident;
    let corebc_core = corebc_core_crate();

    quote! {
        impl #corebc_core::abi::AbiDecode for #name {
            fn decode(bytes: impl AsRef<[u8]>) -> ::core::result::Result<Self, #corebc_core::abi::AbiError> {
                fn _decode(bytes: &[u8]) -> ::core::result::Result<#name, #corebc_core::abi::AbiError> {
                    let #corebc_core::abi::ParamType::Tuple(params) =
                        <#name as #corebc_core::abi::AbiType>::param_type() else { unreachable!() };
                    let min_len = params.iter().map(#corebc_core::abi::minimum_size).sum();
                    if bytes.len() < min_len {
                        Err(#corebc_core::abi::AbiError::DecodingError(#corebc_core::abi::ethabi::Error::InvalidData))
                    } else {
                        let tokens = #corebc_core::abi::decode(&params, bytes)?;
                        let tuple = #corebc_core::abi::Token::Tuple(tokens);
                        let this = <#name as #corebc_core::abi::Tokenizable>::from_token(tuple)?;
                        Ok(this)
                    }
                }

                _decode(bytes.as_ref())
            }
        }

        impl #corebc_core::abi::AbiEncode for #name {
            fn encode(self) -> ::std::vec::Vec<u8> {
                let tokens = #corebc_core::abi::Tokenize::into_tokens(self);
                #corebc_core::abi::encode(&tokens)
            }
        }
    }
}
