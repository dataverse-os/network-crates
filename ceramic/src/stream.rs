use dataverse_types::ceramic::{StreamId, StreamState};
use int_enum::IntEnum;

use crate::event::Event;

#[async_trait::async_trait]
pub trait EventsLoader: Sync + Send {
    async fn load_events(&self, stream_id: &StreamId) -> anyhow::Result<Vec<Event>>;

    async fn load_stream(&self, stream_id: &StreamId) -> anyhow::Result<StreamState> {
        let events = self.load_events(stream_id).await?;

        let mut stream_state: StreamState = StreamState {
            r#type: stream_id.r#type.int_value(),
            ..Default::default()
        };

        for event in events {
            event.apply_to(&mut stream_state)?;
        }

        Ok(stream_state)
    }
}

#[async_trait::async_trait]
impl EventsLoader for super::http::Client {
    async fn load_events(&self, stream_id: &StreamId) -> anyhow::Result<Vec<Event>> {
        self.load_commits(stream_id).await
    }
}
