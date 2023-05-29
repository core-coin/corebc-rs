use crate::*;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_balance_success() {
    run_with_client(Network::Devin, |client| async move {
        let balance = client
            .get_balance(&"ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse().unwrap())
            .await;
        balance.unwrap();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_balance_error() {
    run_with_client(Network::Devin, |client| async move {
        let balance = client
            .get_balance(&"ae654efcf28707488885abbe9d1fc80cbe6d6036f250".parse().unwrap())
            .await;
        balance.unwrap_err();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_transactions_success() {
    run_with_client(Network::Devin, |client| async move {
        let txs = client
            .get_transactions(
                &"ab654efcf28707488885abbe9d1fc80cbe6d6036f250".parse().unwrap(),
                None,
            )
            .await;
        txs.unwrap();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_transactions_error() {
    run_with_client(Network::Devin, |client| async move {
        let txs = client
            .get_transactions(
                &"ae654efcf28707488885abbe9d1fc80cbe6d6036f250".parse().unwrap(),
                None,
            )
            .await;
        txs.unwrap_err();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_tokens_success() {
    run_with_client(Network::Devin, |client| async move {
        let txs = client
            .get_tokens(&"ab57dde1a47041fc3c570c0318a713128ced55fd2ada".parse().unwrap(), None)
            .await;
        txs.unwrap();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_tokens_error() {
    run_with_client(Network::Devin, |client| async move {
        let txs = client
            .get_tokens(&"ae57dde1a47041fc3c570c0318a713128ced55fd2ada".parse().unwrap(), None)
            .await;
        txs.unwrap_err();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_balance_history_success() {
    run_with_client(Network::Devin, |client| async move {
        let history = client
            .get_balance_history(
                &"ab57dde1a47041fc3c570c0318a713128ced55fd2ada".parse().unwrap(),
                None,
            )
            .await;
        history.unwrap();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_balance_history_empty() {
    run_with_client(Network::Devin, |client| async move {
        let history = client
            .get_balance_history(
                &"ae57dde1a47041fc3c570c0318a713128ced55fd2ada".parse().unwrap(),
                None,
            )
            .await;
        assert_eq!(history.unwrap().len(), 0);
    })
    .await
}
