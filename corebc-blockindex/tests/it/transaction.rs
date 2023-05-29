use crate::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_transaction_success() {
    run_with_client(Network::Devin, |client| async move {
        let tx = client
            .get_transaction(
                "0x9a0516515962331000ab0910b969b94cc63e3254ee36664595085af07815fa31"
                    .parse()
                    .unwrap(),
            )
            .await;

        tx.unwrap();
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
