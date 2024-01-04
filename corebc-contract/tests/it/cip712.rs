use corebc_contract_derive::{Cip712, EthAbiType};
use corebc_core::{
    types::{
        transaction::cip712::{
            CIP712Domain as Domain, Cip712, CIP712_DOMAIN_TYPE_HASH,
            CIP712_DOMAIN_TYPE_HASH_WITH_SALT,
        },
        Address, H176, U256,
    },
    utils::{parse_core, sha3},
};

#[test]
fn derive_cip712() {
    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(
        name = "Radicle",
        version = "1",
        network_id = 1,
        verifying_contract = "0x00000000000000000000000000000000000000000001",
        raw_salt = "0x3000000000000000000000000000000000000000000000000000000000000000"
    )]
    pub struct Puzzle {
        pub organization: H176,
        pub contributor: H176,
        pub commit: String,
        pub project: String,
    }

    let puzzle = Puzzle {
        organization: "00000000000000000000000000000000000000000000"
            .parse::<H176>()
            .expect("failed to parse address"),
        contributor: "00000000000000000000000000000000000000000000"
            .parse::<H176>()
            .expect("failed to parse address"),
        commit: "5693b7019eb3e4487a81273c6f5e1832d77acb53".to_string(),
        project: "radicle-reward".to_string(),
    };

    let hash = puzzle.encode_cip712().expect("failed to encode struct");

    assert_eq!(hash.len(), 32)
}

#[test]
fn struct_hash() {
    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(
        name = "Radicle",
        version = "1",
        network_id = 1,
        verifying_contract = "0x00000000000000000000000000000000000000000001",
        salt = "1234567890"
    )]
    pub struct CIP712Domain {
        name: String,
        version: String,
        network_id: U256,
        verifying_contract: Address,
    }

    let domain = Domain {
        name: Some("Radicle".to_string()),
        version: Some("1".to_string()),
        network_id: Some(U256::from(1)),
        verifying_contract: Some(Address::zero()),
        salt: None,
    };

    let domain_test = CIP712Domain {
        name: "Radicle".to_string(),
        version: "1".to_string(),
        network_id: U256::from(1),
        verifying_contract: H176::from(&[0; 22]),
    };

    assert_eq!(CIP712_DOMAIN_TYPE_HASH, CIP712Domain::type_hash().unwrap());

    assert_eq!(domain.separator(), domain_test.struct_hash().unwrap());
}

#[test]
fn derive_cip712_nested() {
    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(
        name = "MyDomain",
        version = "1",
        network_id = 1,
        verifying_contract = "0x00000000000000000000000000000000000000000001"
    )]
    pub struct MyStruct {
        foo: String,
        bar: U256,
        addr: Address,
        /* #[cip712] // Todo: Support nested Cip712 structs
         * nested: MyNestedStruct, */
    }

    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(
        name = "MyDomain",
        version = "1",
        network_id = 1,
        verifying_contract = "0x00000000000000000000000000000000000000000001"
    )]
    pub struct MyNestedStruct {
        foo: String,
        bar: U256,
        addr: Address,
    }

    let my_struct = MyStruct {
        foo: "foo".to_string(),
        bar: U256::from(1),
        addr: Address::from(&[0; 22]),
        /* nested: MyNestedStruct {
         *     foo: "foo".to_string(),
         *     bar: U256::from(1),
         *     addr: Address::from(&[0; 22]),
         * }, */
    };

    let hash = my_struct.struct_hash().expect("failed to hash struct");

    assert_eq!(hash.len(), 32)
}

#[test]
fn uniswap_v2_permit_hash() {
    // See examples/permit_hash.rs for comparison
    // the following produces the same permit_hash as in the example

    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(
        name = "Uniswap V2",
        version = "1",
        network_id = 1,
        verifying_contract = "0x0000B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"
    )]
    struct Permit {
        owner: Address,
        spender: Address,
        value: U256,
        nonce: U256,
        deadline: U256,
    }

    let permit = Permit {
        owner: "0x0000617072Cb2a1897192A9d301AC53fC541d35c4d9D".parse().unwrap(),
        spender: "0x00002819c144D5946404C0516B6f817a960dB37D4929".parse().unwrap(),
        value: parse_core(10).unwrap(),
        nonce: U256::from(1),
        deadline: U256::from(3133728498_u32),
    };

    let permit_hash = permit.encode_cip712().unwrap();

    assert_eq!(
        hex::encode(permit_hash),
        "7511de75cfe7c3de726371ad9f0a703f8b9f0bac0354676114cc5a90f21dc7a8"
    );
}

#[test]
fn domain_hash_constants() {
    assert_eq!(
        CIP712_DOMAIN_TYPE_HASH,
        sha3(
            "CIP712Domain(string name,string version,uint256 networkId,address verifyingContract)"
        )
    );
    assert_eq!(
        CIP712_DOMAIN_TYPE_HASH_WITH_SALT,
        sha3("CIP712Domain(string name,string version,uint256 networkId,address verifyingContract,bytes32 salt)")
    );
}

// https://t.me/ethers_rs/26844
#[test]
fn raw_ident_fields() {
    #[derive(Debug, Clone, Cip712, EthAbiType)]
    #[cip712(name = "replica", version = "1", network_id = 6666)]
    pub struct Message {
        pub title: String,
        pub href: String,
        pub r#type: String,
        pub timestamp: U256,
    }

    assert_eq!(
        Message::type_hash().unwrap(),
        sha3("Message(string title,string href,string type,uint256 timestamp)")
    );
}
