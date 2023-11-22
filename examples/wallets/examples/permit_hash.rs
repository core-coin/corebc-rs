use corebc::{
    contract::{Cip712, EthAbiType},
    core::{
        types::{transaction::cip712::Cip712, Address, U256},
        utils::hex,
    },
};

// Generate the CIP712 permit hash to sign for a Uniswap V2 pair.
// <https://eips.ethereum.org/EIPS/Cip-712>
// <https://eips.ethereum.org/EIPS/eip-2612>
#[derive(Cip712, EthAbiType, Clone)]
#[cip712(
    name = "Uniswap V2",
    version = "1",
    network_id = 1,
    verifying_contract = "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"
)]
struct Permit {
    owner: Address,
    spender: Address,
    value: U256,
    nonce: U256,
    deadline: U256,
}

fn main() {
    let permit = Permit {
        owner: Address::random(),
        spender: Address::random(),
        value: 100.into(),
        nonce: 0.into(),
        deadline: U256::MAX,
    };
    let permit_hash = permit.encode_cip712().unwrap();
    println!("Permit hash: 0x{}", hex::encode(permit_hash));
}
