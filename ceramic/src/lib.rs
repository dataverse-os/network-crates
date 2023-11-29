pub mod did;
pub mod event;
pub mod http;
pub mod kubo;
pub mod network;
pub mod stream;

pub use ceramic_core::StreamId;
pub use event::commit;
pub use stream::*;

pub struct Ceramic {
    pub endpoint: String,
    pub network: network::Network,
}

impl Ceramic {
    pub async fn new(endpoint: &str) -> anyhow::Result<Self> {
        let network = http::Client::network(endpoint).await?;
        Ok(Self {
            endpoint: endpoint.to_string(),
            network,
        })
    }
}
