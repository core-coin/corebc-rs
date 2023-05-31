use crate::*;
use serial_test::serial;
use corebc_core::types::H256;

#[tokio::test]
#[serial]
async fn get_transaction_success() {
    run_with_client(Network::Devin, |client| async move {
        let hash:H256 = "0x9a0516515962331000ab0910b969b94cc63e3254ee36664595085af07815fa31".parse().unwrap();
        let tx = client.get_transaction(hash.clone()).await.unwrap();
        assert_eq!(tx.block_height, 4483929);
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_transaction_error() {
    run_with_client(Network::Devin, |client| async move {
        let tx = client
            .get_transaction(
                "0x8a0516515962331000ab0910b969b94cc63e3254ee36664595085af07815fa31"
                    .parse()
                    .unwrap(),
            )
            .await;

        tx.unwrap_err();
    })
    .await
}
