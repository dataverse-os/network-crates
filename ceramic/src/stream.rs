use anyhow::Result;
use async_trait::async_trait;
use ceramic_core::StreamId;
use dataverse_types::ceramic::StreamState;
use int_enum::IntEnum;

use super::Event;

#[async_trait]
pub trait EventsLoader: Sync {
    async fn load_events(&self, stream_id: &StreamId) -> Result<Vec<Event>>;

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

// #[async_trait]
// impl EventsLoader for super::kubo::Client {
//     async fn load_events(&self, stream_id: &StreamId) -> anyhow::Result<Vec<Event>> {
//         let cid = self.load_last_cid_of_stream(stream_id).await?;
//         let mut events = vec![];
//         let event = self.load_event(cid).await?;
//         match event.prev() {
//             Some(cid) => {
//                 events.push(event);
//                 events.append(&mut self.load_events(stream_id).await?);
//             }
//             None => {}
//         }

//         Ok(events)
//     }
// }
