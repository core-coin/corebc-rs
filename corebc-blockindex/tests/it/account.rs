use crate::*;
use corebc_core::types::Address;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_balance_success() {
    run_with_client(Network::Devin, |client| async move {
        let account: &Address = &"ab77268cebda343475da4384139ad24a90e7afcb80c5".parse().unwrap();
        let balance = client.get_balance(&account.clone()).await.unwrap();
        assert_eq!(balance.clone().balance, "366406336387999999919");
        assert_eq!(balance.account, *account);
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
    run_with_client(Network::Devin, |client: Client| async move {
        let account: &Address = &"ab644ae44561a5a4c4d3011ee104ac8f0f848d84d4dd".parse().unwrap();
        let txs = client.get_transactions(account, None).await;
        assert_eq!(txs.unwrap().len(), 6);
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
// CORETODO: Uncomment test after token fix
// #[tokio::test]
// #[serial]
// async fn get_tokens_success() {
//     let account: &Address = &"ab57dde1a47041fc3c570c0318a713128ced55fd2ada".parse().unwrap();
//     run_with_client(Network::Devin, |client| async move {
//         let tokens = client.get_tokens(account, None).await;
//         assert_eq!(tokens.unwrap().len(), 11);
//     })
//     .await
// }

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
    let account = &"ab77268cebda343475da4384139ad24a90e7afcb80c5".parse().unwrap();
    run_with_client(Network::Devin, |client| async move {
        let history = client.get_balance_history(account, None).await;
        assert_eq!(history.unwrap().len(), 2);
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_balance_history_empty() {
    run_with_client(Network::Devin, |client| async move {
        let history = client
            .get_balance_history(
                &"ab720000000000000000000000000000000000000000".parse().unwrap(),
                None,
            )
            .await;
        assert_eq!(history.unwrap().len(), 0);
    })
    .await
}
