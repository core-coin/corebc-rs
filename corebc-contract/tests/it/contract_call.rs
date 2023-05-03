use corebc_contract_derive::abigen;
use corebc_core::abi::Address;
use corebc_providers::Provider;
use std::{
    future::{Future, IntoFuture},
    sync::Arc,
};

fn _contract_call_into_future_is_send() {
    abigen!(DsProxyFactory, "./../corebc-middleware/contracts/DSProxyFactory.json");
    let (provider, _) = Provider::mocked();
    let client = Arc::new(provider);
    let contract = DsProxyFactory::new(Address::zero(), client);

    fn is_send<T: Future + Send + 'static>(future: T) -> T {
        future
    }

    is_send(contract.cache().into_future());
}
