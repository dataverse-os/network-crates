use ceramic_core::{Cid, StreamId};

use crate::{event::Event, Ceramic};

#[async_trait::async_trait]
pub trait EventsLoader: Sync + Send {
    async fn load_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>>;
}

#[async_trait::async_trait]
pub trait EventsPublisher: Sync + Send {
    async fn publish_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        events: Vec<Event>,
    ) -> anyhow::Result<()>;
}
