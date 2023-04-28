# corebc-core TODO

Ethereum data types, cryptography and utilities.

It is recommended to use the `utils`, `types` and `abi` re-exports instead of
the `core` module to simplify your imports.

This library provides type definitions for Ethereum's main datatypes along with
other utilities for interacting with the Ethereum ecosystem

For more information, please refer to the [book](https://gakonst.com/ethers-rs).

## Feature flags

- `eip712`: Provides the `Eip712` trait and derive procedural macro for EIP-712 encoding of typed data.

## ABI

This crate re-exports the [`ethabi`](https://docs.rs/ethabi) crate's functions
under the `abi` module, as well as the
[`secp256k1`](https://docs.rs/libsecp256k1) and [`rand`](https://docs.rs/rand)
crates for convenience.

## Utilities

The crate provides utilities for launching local Ethereum testnets by using
`ganache-cli` via the `GanacheBuilder` struct.
