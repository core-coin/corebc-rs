// //! These utils are NOT meant for general usage. They are ONLY meant for use
// //! within this module. They DO NOT perform basic safety checks and may panic
// //! if used incorrectly.

// use std::convert::TryFrom;

// use corebc_core::{
//     k256::{
//         ecdsa::{RecoveryId, Signature as RSig, Signature as KSig, VerifyingKey},
//         FieldBytes,
//     },
//     types::{Address, Network, Signature as EthSig, H160, U256},
//     utils::{sha3, to_ican},
// };
// use rusoto_kms::{GetPublicKeyResponse, SignResponse};

// use crate::aws::AwsSignerError;

// /// Makes a trial recovery to check whether an RSig corresponds to a known
// /// `VerifyingKey`
// fn check_candidate(
//     sig: &RSig,
//     recovery_id: RecoveryId,
//     digest: [u8; 32],
//     vk: &VerifyingKey,
// ) -> bool {
//     VerifyingKey::recover_from_prehash(digest.as_slice(), sig, recovery_id)
//         .map(|key| key == *vk)
//         .unwrap_or(false)
// }

// /// Recover an rsig from a signature under a known key by trial/error
// pub(super) fn sig_from_digest_bytes_trial_recovery(
//     sig: &KSig,
//     digest: [u8; 32],
//     vk: &VerifyingKey,
// ) -> EthSig {
//     let r_bytes: FieldBytes = sig.r().into();
//     let s_bytes: FieldBytes = sig.s().into();
//     let r = U256::from_big_endian(r_bytes.as_slice());
//     let s = U256::from_big_endian(s_bytes.as_slice());

//     if check_candidate(sig, RecoveryId::from_byte(0).unwrap(), digest, vk) {
//         EthSig { r, s, v: 0 }
//     } else if check_candidate(sig, RecoveryId::from_byte(1).unwrap(), digest, vk) {
//         EthSig { r, s, v: 1 }
//     } else {
//         panic!("bad sig");
//     }
// }

// /// Modify the v value of a signature to conform to eip155
// pub(super) fn apply_eip155(sig: &mut EthSig, network_id: u64) {
//     let v = (network_id * 2 + 35) + sig.v;
//     sig.v = v;
// }

// /// Convert a verifying key to an ethereum address
// pub(super) fn verifying_key_to_address(key: &VerifyingKey, network: Network) -> Address {
//     // false for uncompressed
//     let uncompressed_pub_key = key.to_encoded_point(false);
//     let public_key = uncompressed_pub_key.to_bytes();
//     debug_assert_eq!(public_key[0], 0x04);
//     let hash = sha3(&public_key[1..]);

//     let mut bytes = [0u8; 20];
//     bytes.copy_from_slice(&hash[12..]);
//     let addr = H160::from(bytes);

//     to_ican(&addr, &network)
// }

// /// Decode an AWS KMS Pubkey response
// pub(super) fn decode_pubkey(resp: GetPublicKeyResponse) -> Result<VerifyingKey, AwsSignerError> {
//     let raw = resp
//         .public_key
//         .ok_or_else(|| AwsSignerError::from("Pubkey not found in response".to_owned()))?;

//     let spki = spki::SubjectPublicKeyInfoRef::try_from(raw.as_ref())?;
//     let key = VerifyingKey::from_sec1_bytes(spki.subject_public_key.raw_bytes())?;

//     Ok(key)
// }

// /// Decode an AWS KMS Signature response
// pub(super) fn decode_signature(resp: SignResponse) -> Result<KSig, AwsSignerError> {
//     let raw = resp
//         .signature
//         .ok_or_else(|| AwsSignerError::from("Signature not found in response".to_owned()))?;

//     let sig = KSig::from_der(&raw)?;
//     Ok(sig.normalize_s().unwrap_or(sig))
// }
