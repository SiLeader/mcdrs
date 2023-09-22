use endpoint::start_server;
use std::sync::Arc;
use storage::HashMapStorage;

#[tokio::main]
async fn main() {
    println!("Hello, world!");
    env_logger::init();

    let mem_storage = HashMapStorage::default();

    start_server(("localhost", 11211), Arc::new(mem_storage))
        .await
        .unwrap();
}
