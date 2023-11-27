mod commit_id;
mod patch;
mod stream;
mod stream_id;

pub use stream::*;
pub use stream_id::*;

use ceramic_core::StreamId;
use int_enum::IntEnum;

use crate::{
    commit::{Data, Genesis},
    event::Event,
};

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

#[async_trait::async_trait]
pub trait EventsPublisher: Sync + Send {
    async fn publish_events(
        &self,
        network: String,
        stream_id: &StreamId,
        events: Vec<Event>,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl EventsPublisher for super::http::Client {
    async fn publish_events(
        &self,
        _network: String,
        stream_id: &StreamId,
        commits: Vec<Event>,
    ) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        for ele in &commits {
            match ele.log_type() {
                LogType::Genesis => {
                    let url = self.ceramic.url_for_path("/api/v0/streams")?;
                    let genesis = Genesis {
                        r#type: stream_id.r#type.int_value(),
                        genesis: ele.clone().try_into()?,
                        opts: serde_json::Value::Null,
                    };
                    match client.post(url.as_str()).json(&genesis).send().await {
                        Ok(res) => log::debug!("publish genesis {:?}", res),
                        Err(err) => log::error!("publish genesis {}", err),
                    };
                }
                LogType::Signed => {
                    let url = self.ceramic.url_for_path("/api/v0/commits")?;
                    let signed = Data {
                        stream_id: stream_id.clone(),
                        commit: ele.clone().try_into()?,
                        opts: serde_json::Value::Null,
                    };
                    match client.post(url.as_str()).json(&signed).send().await {
                        Ok(res) => log::debug!("publish signed {:?}", res),
                        Err(err) => log::error!("publish signed {}", err),
                    };
                }
                _ => anyhow::bail!("invalid log type"),
            };
        }
        Ok(())
    }
}
