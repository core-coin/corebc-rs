use corebc_core::types::{Network, H160};

/// The Multicall3 contract address that is deployed in [`MULTICALL_SUPPORTED_NETWORK_IDS`]:
/// [`0xcA11bde05977b3631167028862bE2a173976CA11`](https://etherscan.io/address/0xcA11bde05977b3631167028862bE2a173976CA11)
pub const MULTICALL_ADDRESS: H160 = H160([
    0xca, 0x11, 0xbd, 0xe0, 0x59, 0x77, 0xb3, 0x63, 0x11, 0x67, 0x02, 0x88, 0x62, 0xbe, 0x2a, 0x17,
    0x39, 0x76, 0xca, 0x11,
]);

/// The network IDs that [`MULTICALL_ADDRESS`] has been deployed to.
///
/// Taken from: <https://github.com/mds1/multicall#multicall3-contract-addresses>
pub const MULTICALL_SUPPORTED_NETWORK_IDS: &[u64] = {
    use Network::*;
    &[
        Mainnet as u64,                  // Mainnet
        Devin as u64,                    // Devin
    ]
};
