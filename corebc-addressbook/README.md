# corebc-addressbook

A collection of commonly used smart contract addresses.

For more information, please refer to the [book](https://gakonst.com/ethers-rs).

## Examples

```rust
use corebc_addressbook::{contract, Network};

let ctn = contract("ctn").unwrap();
let mainnet_address = ctn.address(Network::Mainnet).unwrap();
assert_eq!(mainnet_address, "0xcb19c7acc4c292d2943ba23c2eaa5d9c5a6652a8710c".parse().unwrap());
```
