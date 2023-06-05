# corebc-blockindex

Bindings for the [blockindex.net web API](https://blockindex.net/).

## Examples

```rust,no_run
# use corebc_core::types::Network;
# use corebc_blockindex::Client;
# async fn foo() -> Result<(), Box<dyn std::error::Error>> {
let client = Client::new(Network::Devin)?;

let address = &"ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse()?;
let balance = client.get_balance(address).await?;
assert_eq!(balance.balance, "10000000");
# Ok(())
# }
```
