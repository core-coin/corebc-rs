use hex::{FromHex, ToHex};
use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};
use uuid::Uuid;

use corebc_core::abi::ethereum_types::H176 as Address;

#[derive(Debug, Deserialize, Serialize)]
/// This struct represents the deserialized form of an encrypted JSON keystore based on the
/// [Web3 Secret Storage Definition](https://github.com/ethereum/wiki/wiki/Web3-Secret-Storage-Definition).
pub struct EthKeystore {
    pub address: Address,

    pub crypto: CryptoJson,
    pub id: Uuid,
    pub version: u8,
}

#[derive(Debug, Deserialize, Serialize)]
/// Represents the "crypto" part of an encrypted JSON keystore.
pub struct CryptoJson {
    pub cipher: String,
    pub cipherparams: CipherparamsJson,
    #[serde(serialize_with = "buffer_to_hex", deserialize_with = "hex_to_buffer")]
    pub ciphertext: Vec<u8>,
    pub kdf: KdfType,
    pub kdfparams: KdfparamsType,
    #[serde(serialize_with = "buffer_to_hex", deserialize_with = "hex_to_buffer")]
    pub mac: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
/// Represents the "cipherparams" part of an encrypted JSON keystore.
pub struct CipherparamsJson {
    #[serde(serialize_with = "buffer_to_hex", deserialize_with = "hex_to_buffer")]
    pub iv: Vec<u8>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
/// Types of key derivition functions supported by the Web3 Secret Storage.
pub enum KdfType {
    Pbkdf2,
    Scrypt,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
/// Defines the various parameters used in the supported KDFs.
pub enum KdfparamsType {
    Pbkdf2 {
        c: u32,
        dklen: u8,
        prf: String,
        #[serde(serialize_with = "buffer_to_hex", deserialize_with = "hex_to_buffer")]
        salt: Vec<u8>,
    },
    Scrypt {
        dklen: u8,
        n: u32,
        p: u32,
        r: u32,
        #[serde(serialize_with = "buffer_to_hex", deserialize_with = "hex_to_buffer")]
        salt: Vec<u8>,
    },
}

fn buffer_to_hex<T, S>(buffer: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&buffer.encode_hex::<String>())
}

fn hex_to_buffer<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    String::deserialize(deserializer)
        .and_then(|string| Vec::from_hex(string).map_err(|err| Error::custom(err.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_gocore_compat_keystore() {
        let data = r#"
        {
            "address": "cb27de521e43741cf785cbad450d5649187b9612018f",
            "crypto": {
              "cipher": "aes-128-ctr",
              "ciphertext": "13b3adc4203f236343e7c6a59c04ac56e6dee12258eba391697d2b5f1e9b87698f53ae12b5837bc906c42bce38f9b575ea82dfaef84b1d6795",
              "cipherparams": {
                "iv": "ce4b6edebbcb44c76ba42cb2c7a59dd3"
              },
              "kdf": "scrypt",
              "kdfparams": {
                "dklen": 32,
                "n": 2,
                "p": 1,
                "r": 8,
                "salt": "54eeca614269586df5dfbde87b39e4077b0353d23fe3fb0e72312973b8d4d3df"
              },
              "mac": "d7450eab867cc3b4f7619a586878428cf2a73056fcae32cc605df8ce07e799f6"
            },
            "id": "84c5fc55-b541-42ed-aaa7-28f508cd85c0",
            "version": 3
          }"#;
        let keystore: EthKeystore = serde_json::from_str(data).unwrap();
        assert_eq!(
            keystore.address.as_bytes().to_vec(),
            hex::decode("cb27de521e43741cf785cbad450d5649187b9612018f").unwrap()
        );
    }

    // #[test]
    // fn test_deserialize_pbkdf2() {
    //     let data = r#"
    //     {
    //         "crypto" : {
    //             "cipher" : "aes-128-ctr",
    //             "cipherparams" : {
    //                 "iv" : "6087dab2f9fdbbfaddc31a909735c1e6"
    //             },
    //             "ciphertext" :
    // "5318b4d5bcd28de64ee5559e671353e16f075ecae9f99c7a79a38af5f869aa46",             "kdf" :
    // "pbkdf2",             "kdfparams" : {
    //                 "c" : 262144,
    //                 "dklen" : 32,
    //                 "prf" : "hmac-sha256",
    //                 "salt" : "ae3cd4e7013836a3df6bd7241b12db061dbe2c6785853cce422d148a624ce0bd"
    //             },
    //             "mac" : "517ead924a9d0dc3124507e3393d175ce3ff7c1e96529c6c555ce9e51205e9b2"
    //         },
    //         "id" : "3198bc9c-6672-5ab3-d995-4942343ae5b6",
    //         "version" : 3
    //     }"#;
    //     let keystore: EthKeystore = serde_json::from_str(data).unwrap();
    //     assert_eq!(keystore.version, 3);
    //     assert_eq!(
    //         keystore.id,
    //         Uuid::parse_str("3198bc9c-6672-5ab3-d995-4942343ae5b6").unwrap()
    //     );
    //     assert_eq!(keystore.crypto.cipher, "aes-128-ctr");
    //     assert_eq!(
    //         keystore.crypto.cipherparams.iv,
    //         Vec::from_hex("6087dab2f9fdbbfaddc31a909735c1e6").unwrap()
    //     );
    //     assert_eq!(
    //         keystore.crypto.ciphertext,
    //         Vec::from_hex("5318b4d5bcd28de64ee5559e671353e16f075ecae9f99c7a79a38af5f869aa46")
    //             .unwrap()
    //     );
    //     assert_eq!(keystore.crypto.kdf, KdfType::Pbkdf2);
    //     assert_eq!(
    //         keystore.crypto.kdfparams,
    //         KdfparamsType::Pbkdf2 {
    //             c: 262144,
    //             dklen: 32,
    //             prf: String::from("hmac-sha256"),
    //             salt: Vec::from_hex(
    //                 "ae3cd4e7013836a3df6bd7241b12db061dbe2c6785853cce422d148a624ce0bd"
    //             )
    //             .unwrap(),
    //         }
    //     );
    //     assert_eq!(
    //         keystore.crypto.mac,
    //         Vec::from_hex("517ead924a9d0dc3124507e3393d175ce3ff7c1e96529c6c555ce9e51205e9b2")
    //             .unwrap()
    //     );
    // }

    #[test]
    fn test_deserialize_scrypt() {
        let data = r#"
        {
            "address": "cb27de521e43741cf785cbad450d5649187b9612018f",
            "crypto": {
              "cipher": "aes-128-ctr",
              "ciphertext": "13b3adc4203f236343e7c6a59c04ac56e6dee12258eba391697d2b5f1e9b87698f53ae12b5837bc906c42bce38f9b575ea82dfaef84b1d6795",
              "cipherparams": {
                "iv": "ce4b6edebbcb44c76ba42cb2c7a59dd3"
              },
              "kdf": "scrypt",
              "kdfparams": {
                "dklen": 32,
                "n": 2,
                "p": 1,
                "r": 8,
                "salt": "54eeca614269586df5dfbde87b39e4077b0353d23fe3fb0e72312973b8d4d3df"
              },
              "mac": "d7450eab867cc3b4f7619a586878428cf2a73056fcae32cc605df8ce07e799f6"
            },
            "id": "84c5fc55-b541-42ed-aaa7-28f508cd85c0",
            "version": 3
          }"#;
        let keystore: EthKeystore = serde_json::from_str(data).unwrap();
        assert_eq!(keystore.version, 3);
        assert_eq!(keystore.id, Uuid::parse_str("84c5fc55-b541-42ed-aaa7-28f508cd85c0").unwrap());
        assert_eq!(keystore.crypto.cipher, "aes-128-ctr");
        assert_eq!(
            keystore.crypto.cipherparams.iv,
            Vec::from_hex("ce4b6edebbcb44c76ba42cb2c7a59dd3").unwrap()
        );
        assert_eq!(
            keystore.crypto.ciphertext,
            Vec::from_hex("13b3adc4203f236343e7c6a59c04ac56e6dee12258eba391697d2b5f1e9b87698f53ae12b5837bc906c42bce38f9b575ea82dfaef84b1d6795")
                .unwrap()
        );
        assert_eq!(keystore.crypto.kdf, KdfType::Scrypt);
        assert_eq!(
            keystore.crypto.kdfparams,
            KdfparamsType::Scrypt {
                dklen: 32,
                n: 2,
                p: 1,
                r: 8,
                salt: Vec::from_hex(
                    "54eeca614269586df5dfbde87b39e4077b0353d23fe3fb0e72312973b8d4d3df"
                )
                .unwrap(),
            }
        );
        assert_eq!(
            keystore.crypto.mac,
            Vec::from_hex("d7450eab867cc3b4f7619a586878428cf2a73056fcae32cc605df8ce07e799f6")
                .unwrap()
        );
    }
}
