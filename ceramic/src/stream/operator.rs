use ceramic_core::StreamId;

use crate::{event::Event, network::Network};

#[async_trait::async_trait]
pub trait EventsLoader: Sync + Send {
    async fn load_events(&self, stream_id: &StreamId) -> anyhow::Result<Vec<Event>>;
}

#[async_trait::async_trait]
pub trait EventsPublisher: Sync + Send {
    async fn publish_events(
        &self,
        network: Network,
        stream_id: &StreamId,
        events: Vec<Event>,
    ) -> anyhow::Result<()>;
}
