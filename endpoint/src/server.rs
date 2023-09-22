use crate::handle_socket::handle_socket;
use crate::handler::MemcachedHandler;
use log::info;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};

pub async fn start_server<A: ToSocketAddrs>(
    address: A,
    handler: Arc<dyn MemcachedHandler>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(address).await?;

    loop {
        let (socket, peer_address) = listener.accept().await?;
        info!("Accept socket peer address is {peer_address}");
        let processor_handler = handler.clone();
        tokio::spawn(async move { handle_socket(socket, processor_handler).await });
    }
}
