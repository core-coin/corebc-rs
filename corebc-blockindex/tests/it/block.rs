use crate::*;
use corebc_blockindex::block::BlockQueryOption;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_block_by_number() {
    run_with_client(Network::Devin, |client| async move {
        let block = client.get_block(BlockQueryOption::ByNumber(289632)).await;
        assert_eq!(
            block.unwrap().hash,
            "0xa9b9902f750ebde2179c4cec87c53a50eaf3ce6e8834570b8935e9e063e88303".parse().unwrap()
        );
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_block_by_hash() {
    run_with_client(Network::Devin, |client| async move {
        let block = client
            .get_block(BlockQueryOption::ByHash(
                "0xa9b9902f750ebde2179c4cec87c53a50eaf3ce6e8834570b8935e9e063e88303".to_string(),
            ))
            .await;
        assert_eq!(block.unwrap().height, 289632);
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_block_error() {
    run_with_client(Network::Devin, |client| async move {
        let block = client.get_block(BlockQueryOption::ByHash("0x0asdf".to_string())).await;
        block.unwrap_err();
    })
    .await
}
