use crate::frame::{MemcachedCodec, MemcachedRequest, MemcachedResponse};
use crate::handler::MemcachedHandler;
use crate::MemcachedError;
use futures::SinkExt;
use log::{debug, trace, warn};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

pub(super) async fn handle_socket(socket: TcpStream, handler: Arc<dyn MemcachedHandler>) {
    match handle_socket_impl(socket, handler).await {
        Ok(_) => debug!("Handle request success"),
        Err(e) => warn!("Handle request error: {e}"),
    }
}

async fn handle_socket_impl(
    socket: TcpStream,
    handler: Arc<dyn MemcachedHandler>,
) -> std::io::Result<()> {
    let mut framed = Framed::new(socket, MemcachedCodec::default());

    while let Some(request) = framed.next().await {
        let request = match request {
            Ok(r) => r,
            Err(e) => {
                warn!("Invalid input data: {e}");
                let res = Err(MemcachedError::Client("Invalid data".to_string()));
                framed.send(res).await?;
                continue;
            }
        };
        trace!("Request handling: {:?}", request);

        let res = match request {
            MemcachedRequest::Set {
                key,
                value,
                options,
            } => handler.set(key, value, options).await,
            MemcachedRequest::Add {
                key,
                value,
                options,
            } => handler.add(key, value, options).await,
            MemcachedRequest::Replace {
                key,
                value,
                options,
            } => handler.replace(key, value, options).await,
            MemcachedRequest::Append {
                key,
                value,
                options,
            } => handler.append(key, value, options).await,
            MemcachedRequest::Prepend {
                key,
                value,
                options,
            } => handler.prepend(key, value, options).await,
            MemcachedRequest::Get { key } => handler.get(key).await,
            MemcachedRequest::Delete { key } => handler.delete(key).await,
            MemcachedRequest::Incr { key, diff } => handler.increment(key, diff).await,
            MemcachedRequest::Decr { key, diff } => handler.decrement(key, diff).await,
            MemcachedRequest::Stats => handler.statistics().await,
            MemcachedRequest::Version => Ok(MemcachedResponse::Version("0.1.0".to_string())),
            MemcachedRequest::Unsupported => Err(MemcachedError::NoExistenceCommand),
        };
        framed.send(res).await?;
    }

    Ok(())
}
