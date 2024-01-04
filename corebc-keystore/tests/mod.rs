use corebc_keystore::{decrypt_key, encrypt_key, new};
use hex::FromHex;
use std::path::Path;

mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let dir = Path::new("./tests/test-keys");
        let mut rng = rand::thread_rng();
        let (secret, id) = new(
            &dir,
            &mut rng,
            "thebestrandompassword",
            None,
            &corebc_core::types::Network::Mainnet,
        )
        .unwrap();

        let keypath = dir.join(&id);

        assert_eq!(decrypt_key(&keypath, "thebestrandompassword").unwrap(), secret);
        assert!(decrypt_key(&keypath, "notthebestrandompassword").is_err());
        assert!(std::fs::remove_file(&keypath).is_ok());
    }

    #[test]
    fn test_new_with_name() {
        let dir = Path::new("./tests/test-keys");
        let mut rng = rand::thread_rng();
        let name = "my_keystore";
        let (secret, _id) = new(
            &dir,
            &mut rng,
            "thebestrandompassword",
            Some(name),
            &corebc_core::types::Network::Mainnet,
        )
        .unwrap();

        let keypath = dir.join(&name);

        assert_eq!(decrypt_key(&keypath, "thebestrandompassword").unwrap(), secret);
        assert!(std::fs::remove_file(&keypath).is_ok());
    }

    // #[test]
    // fn test_decrypt_pbkdf2() {
    //     let secret =
    //         Vec::from_hex("7a28b5ba57c53603b0b07b56bba752f7784bf506fa95edc395f5cf6c7514fe9d")
    //             .unwrap();
    //     let keypath = Path::new("./tests/test-keys/key-pbkdf2.json");
    //     assert_eq!(decrypt_key(&keypath, "testpassword").unwrap(), secret);
    //     assert!(decrypt_key(&keypath, "wrongtestpassword").is_err());
    // }

    #[test]
    fn test_decrypt_scrypt() {
        let secret =
            Vec::from_hex("76e6c724489736e6107e28b505c0ba6021d75b26f0bbbafe01609f6dedc92d1078d2392e75b828cc668ef3662486403cd617622363fb5298a9")
                .unwrap();
        let keypath = Path::new("./tests/test-keys/key-scrypt.json");
        assert_eq!(decrypt_key(&keypath, "foobar").unwrap(), secret);
        assert!(decrypt_key(&keypath, "thisisnotrandom").is_err());
    }

    #[test]
    fn test_encrypt_decrypt_key() {
        let secret =
            Vec::from_hex("76e6c724489736e6107e28b505c0ba6021d75b26f0bbbafe01609f6dedc92d1078d2392e75b828cc668ef3662486403cd617622363fb5298a9")
                .unwrap();
        let dir = Path::new("./tests/test-keys");
        let mut rng = rand::thread_rng();
        let name = encrypt_key(
            &dir,
            &mut rng,
            &secret,
            "newpassword",
            None,
            &corebc_core::types::Network::Mainnet,
        )
        .unwrap();

        let keypath = dir.join(&name);
        assert_eq!(decrypt_key(&keypath, "newpassword").unwrap(), secret);
        assert!(decrypt_key(&keypath, "notanewpassword").is_err());
        assert!(std::fs::remove_file(&keypath).is_ok());
    }
}
