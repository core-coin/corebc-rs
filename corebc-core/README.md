# corebc-core TODO

Core data types, cryptography and utilities.

It is recommended to use the `utils`, `types` and `abi` re-exports instead of
the `core` module to simplify your imports.

This library provides type definitions for Core's main datatypes along with
other utilities for interacting with the Core ecosystem

## Feature flags

- `eip712`: Provides the `Eip712` trait and derive procedural macro for EIP-712 encoding of typed data.

## ABI

This crate re-exports the [`ethabi`](https://docs.rs/ethabi) crate's functions
under the `abi` module, as well as the
[`secp256k1`](https://docs.rs/libsecp256k1) and [`rand`](https://docs.rs/rand)
crates for convenience.

