use crate::*;
use corebc_blockindex::block::BlockQueryOption;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn get_block_by_number() {
    run_with_client(Network::Devin, |client| async move {
        let block = client.get_block(BlockQueryOption::ByNumber(4483929)).await;
        block.unwrap();
    })
    .await
}

#[tokio::test]
#[serial]
async fn get_block_by_hash() {
    run_with_client(Network::Devin, |client| async move {
        let block = client
            .get_block(BlockQueryOption::ByHash(
                "0x77a1a8214e05ba5e48f88a7a2f4cc25e65dde772aa23ad6efa03f95c8a4d35bb".to_string(),
            ))
            .await;
        block.unwrap();
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
