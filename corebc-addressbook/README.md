# corebc-addressbook

A collection of commonly used smart contract addresses.

For more information, please refer to the [book](https://gakonst.com/ethers-rs).

## Examples

```rust
use corebc_addressbook::{contract, Network};

let weth = contract("weth").unwrap();
let mainnet_address = weth.address(Network::Mainnet).unwrap();
assert_eq!(mainnet_address, "0x0000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".parse().unwrap());
```
