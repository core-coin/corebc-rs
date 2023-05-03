use ethers::{
    contract::{abigen, ContractFactory},
    core::utils::Anvil,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::{LocalWallet, Signer},
    solc::Solc,
    types::{Address, U256},
    utils::AnvilInstance,
};
use eyre::Result;
use std::{convert::TryFrom, path::Path, sync::Arc, time::Duration};

#[tokio::main]
async fn main() -> Result<()> {
    // Start a fresh local Anvil node to execute the transfer on.
    let anvil = Anvil::new().spawn();
    // Use the first wallet autogenerated by the Anvil node to send the transaction.
    let from_wallet: LocalWallet = anvil.keys()[0].clone().into();
    let to_address = anvil.addresses()[2].clone();

    // Deploy an ERC20 token contract on the freshly started local Anvil node that doesn't have any
    // tokens deployed on it. This will also allocate tokens to the `from_wallet` address.
    // You don't need to do this to transfer an already deployed ERC20 token, in that case you only
    // need the token address.
    let token_address = deploy_token_contract(&anvil, from_wallet.clone()).await?;

    // 1. Generate the ABI for the ERC20 contract. This is will define an `ERC20Contract` struct in
    // this scope that will let us call the methods of the contract.
    abigen!(
        ERC20Contract,
        r#"[
            function balanceOf(address account) external view returns (uint256)
            function decimals() external view returns (uint8)
            function symbol() external view returns (string memory)
            function transfer(address to, uint256 amount) external returns (bool)
            event Transfer(address indexed from, address indexed to, uint256 value)
        ]"#,
    );

    // 2. Create the contract instance to let us call methods of the contract and let it sign
    // transactions with the sender wallet.
    let provider =
        Provider::<Http>::try_from(anvil.endpoint())?.interval(Duration::from_millis(10u64));
    let signer =
        Arc::new(SignerMiddleware::new(provider, from_wallet.with_network_id(anvil.network_id())));
    let contract = ERC20Contract::new(token_address, signer);

    // 3. Fetch the decimals used by the contract so we can compute the decimal amount to send.
    let whole_amount: u64 = 1000;
    let decimals = contract.decimals().call().await?;
    let decimal_amount = U256::from(whole_amount) * U256::exp10(decimals as usize);

    // 4. Transfer the desired amount of tokens to the `to_address`
    let tx = contract.transfer(to_address, decimal_amount);
    let pending_tx = tx.send().await?;
    let _mined_tx = pending_tx.await?;

    // 5. Fetch the balance of the recipient.
    let balance = contract.balance_of(to_address).call().await?;
    assert_eq!(balance, decimal_amount);

    Ok(())
}

/// Helper function to deploy a contract on a local anvil node. See
/// `examples/contracts/examples/deploy_from_solidity.rs` for detailed explanation on how to deploy
/// a contract.
/// Allocates tokens to the deployer address and returns the ERC20 contract address.
async fn deploy_token_contract(
    anvil: &AnvilInstance,
    deployer_wallet: LocalWallet,
) -> Result<Address> {
    let source = Path::new(&env!("CARGO_MANIFEST_DIR"))
        .join("examples/contracts/erc20_example/ERC20Example.sol");
    let compiled = Solc::default().compile_source(source).expect("Could not compile contract");
    let (abi, bytecode, _runtime_bytecode) =
        compiled.find("ERC20Example").expect("could not find contract").into_parts_or_default();

    let provider =
        Provider::<Http>::try_from(anvil.endpoint())?.interval(Duration::from_millis(10u64));
    let client = SignerMiddleware::new(provider, deployer_wallet.with_network_id(anvil.network_id()));
    let client = Arc::new(client);
    let factory = ContractFactory::new(abi, bytecode, client.clone());
    let contract = factory.deploy(())?.send().await?;

    Ok(contract.address())
}
