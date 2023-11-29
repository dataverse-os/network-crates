use ceramic_core::{Cid, StreamId};
use dataverse_ceramic::{
    event::{Event, EventsLoader, EventsPublisher},
    Ceramic, StreamLoader, StreamOperator, StreamPublisher, StreamState,
};
use dataverse_file_system::file::StreamFileLoader;

use crate::Client;

impl StreamFileLoader for Client {}

impl StreamOperator for Client {}

#[async_trait::async_trait]
impl StreamLoader for Client {
    async fn load_streams(
        &self,
        ceramic: &Ceramic,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        self.list_stream_states_in_model(ceramic, account, model_id)
            .await
    }
}

impl StreamPublisher for Client {}

#[async_trait::async_trait]
impl EventsPublisher for Client {
    async fn publish_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        events: Vec<Event>,
    ) -> anyhow::Result<()> {
        self.kubo.publish_events(ceramic, stream_id, events).await
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
            None => self.load_stream(stream_id).await?.tip,
        };
        self.kubo.load_events(ceramic, stream_id, Some(tip)).await
    }
}
