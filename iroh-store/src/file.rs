use anyhow::Context;
use ceramic_core::{Cid, StreamId};
use dataverse_ceramic::{
    event::{Event, EventsLoader, EventsUploader},
    Ceramic, StreamOperator, StreamPublisher,
};
use dataverse_core::stream::StreamStore;
use dataverse_file_system::file::StreamFileLoader;

use crate::Client;

impl StreamFileLoader for Client {}

impl StreamOperator for Client {}

#[async_trait::async_trait]
impl EventsUploader for Client {
    async fn upload_event(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()> {
        self.kubo
            .publish_events(ceramic, stream_id, vec![commit])
            .await
    }
}

#[async_trait::async_trait]
impl EventsLoader for Client {
    async fn load_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>> {
        let tip = match tip {
            Some(tip) => tip,
            None => {
                self.load_stream(stream_id)
                    .await?
                    .context(format!("stream not found: {}", stream_id))?
                    .tip
            }
        };
        self.kubo.load_events(ceramic, stream_id, Some(tip)).await
    }
}
