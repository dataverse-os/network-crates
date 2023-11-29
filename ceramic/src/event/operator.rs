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
pub trait EventsUploader {
    async fn upload_event(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()>;

    async fn upload_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Vec<Event>,
    ) -> anyhow::Result<()> {
        for event in commit {
            self.upload_event(ceramic, stream_id, event).await?;
        }
        Ok(())
    }
}
