pub mod gocore_compat {
    use crate::KeystoreError;
    use corebc_core::{
        abi::ethereum_types::{H160, H176 as Address},
        types::Network,
        utils::to_ican,
    };
    use libgoldilocks::SigningKey;
    use tiny_keccak::{Hasher, Sha3};

    /// Converts a K256 SigningKey to an Core Address
    pub fn address_from_pk<S>(pk: S, network: &Network) -> Result<Address, KeystoreError>
    where
        S: AsRef<[u8]>,
    {
        let secret_key = SigningKey::from_bytes(pk.as_ref())?;
        let public_key = secret_key.verifying_key();
        let public_key = public_key.as_bytes();

        let hash = sha3(public_key);
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&hash[12..]);
        let addr = H160::from(bytes);
        Ok(to_ican(&addr, network))
    }

    /// Compute the Keccak-256 hash of input bytes.
    pub fn sha3<T: AsRef<[u8]>>(bytes: T) -> [u8; 32] {
        let mut output = [0u8; 32];

        let mut hasher = Sha3::v256();
        hasher.update(bytes.as_ref());
        hasher.finalize(&mut output);

        output
    }
}
