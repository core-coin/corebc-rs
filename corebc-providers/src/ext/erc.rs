//! ERC related utilities. Only supporting NFTs for now.
use corebc_core::types::{Address, Selector, U256};

use serde::Deserialize;
use std::str::FromStr;
use url::Url;

/// sha3(ownerOf(uint256 tokenId))
pub const ERC721_OWNER_SELECTOR: Selector = [0x37, 0x53, 0xff, 0x5b];

/// sha3(balanceOf(address owner, uint256 tokenId))
pub const ERC1155_BALANCE_SELECTOR: Selector = [0xc5, 0x52, 0x45, 0x46];

const IPFS_GATEWAY: &str = "https://ipfs.io/ipfs/";

/// An ERC 721 or 1155 token
pub struct ERCNFT {
    /// Type of the NFT
    pub type_: ERCNFTType,
    /// Address of the NFT contract
    pub contract: Address,
    /// NFT ID in that contract
    pub id: [u8; 32],
}

impl FromStr for ERCNFT {
    type Err = String;
    fn from_str(input: &str) -> Result<ERCNFT, Self::Err> {
        let split: Vec<&str> =
            input.trim_start_matches("eip155:").trim_start_matches("1/").split(':').collect();
        let (token_type, inner_path) = if split.len() == 2 {
            (
                ERCNFTType::from_str(split[0])
                    .map_err(|_| "Unsupported ERC token type".to_string())?,
                split[1],
            )
        } else {
            return Err("Unsupported ERC link".to_string())
        };

        let token_split: Vec<&str> = inner_path.split('/').collect();
        let (contract_addr, token_id) = if token_split.len() == 2 {
            let token_id = U256::from_dec_str(token_split[1])
                .map_err(|e| format!("Unsupported token id type: {} {e}", token_split[1]))?;
            let mut token_id_bytes = [0x0; 32];
            token_id.to_big_endian(&mut token_id_bytes);
            (
                Address::from_str(token_split[0].trim_start_matches("0x"))
                    .map_err(|e| format!("Invalid contract address: {} {e}", token_split[0]))?,
                token_id_bytes,
            )
        } else {
            return Err("Unsupported ERC link path".to_string())
        };
        Ok(ERCNFT { id: token_id, type_: token_type, contract: contract_addr })
    }
}

/// Supported ERCs
#[derive(PartialEq, Eq)]
pub enum ERCNFTType {
    /// ERC721
    ERC721,
    /// ERC1155
    ERC1155,
}

impl FromStr for ERCNFTType {
    type Err = ();
    fn from_str(input: &str) -> Result<ERCNFTType, Self::Err> {
        match input {
            "erc721" => Ok(ERCNFTType::ERC721),
            "erc1155" => Ok(ERCNFTType::ERC1155),
            _ => Err(()),
        }
    }
}

impl ERCNFTType {
    /// Get the method selector
    pub const fn resolution_selector(&self) -> Selector {
        match self {
            // sha3(tokenURI(uint256))
            ERCNFTType::ERC721 => [0xa8, 0x9d, 0xa6, 0x37],
            // sha3(url(uint256)0
            ERCNFTType::ERC1155 => [0x0d, 0x7c, 0x4c, 0x12],
        }
    }
}

/// ERC-1155 and ERC-721 metadata document.
#[derive(Deserialize)]
pub struct Metadata {
    /// The URL of the image for the NFT
    pub image: String,
}

/// Returns a HTTP url for an IPFS object.
pub fn http_link_ipfs(url: Url) -> Result<Url, String> {
    Url::parse(IPFS_GATEWAY)
        .unwrap()
        .join(url.to_string().trim_start_matches("ipfs://").trim_start_matches("ipfs/"))
        .map_err(|e| e.to_string())
}
