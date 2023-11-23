#![allow(clippy::extra_unused_type_parameters)]
#![cfg(feature = "abigen")]

mod abigen;

mod derive;

mod contract_call;

mod cip712;

#[cfg(all(not(target_arch = "wasm32")))]
mod common;

#[cfg(all(not(target_arch = "wasm32")))]
mod contract;
