use crate::{
    event::{Event, EventsLoader, EventsPublisher},
    Ceramic, StreamState,
};
use ceramic_core::{Cid, StreamId};
use int_enum::IntEnum;

pub trait StreamOperator: StreamLoader + StreamPublisher {}

#[async_trait::async_trait]
pub trait StreamLoader: EventsLoader + Sync + Send {
    async fn load_stream(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<StreamState> {
        let commits = self.load_events(ceramic, stream_id, tip).await?;
        StreamState::new(stream_id.r#type.int_value(), commits)
    }

    async fn load_streams(
        &self,
        ceramic: &Ceramic,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>>;
}

#[async_trait::async_trait]
pub trait StreamPublisher: EventsPublisher + Sync + Send {
    async fn publish_stream(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commits: Vec<Event>,
    ) -> anyhow::Result<()> {
        self.publish_events(ceramic, stream_id, commits).await
    }
}

pub struct CachedStreamLoader<T: StreamLoader> {
    loader: T,
    cache: std::collections::HashMap<String, StreamState>,
}

impl<T: StreamLoader> CachedStreamLoader<T> {
    pub fn new(loader: T) -> Self {
        Self {
            loader,
            cache: std::collections::HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl<T: StreamLoader + Send + Sync> EventsLoader for CachedStreamLoader<T> {
    async fn load_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>> {
        self.loader.load_events(ceramic, stream_id, tip).await
    }
}

#[async_trait::async_trait]
impl<T: StreamLoader + Send + Sync> StreamLoader for CachedStreamLoader<T> {
    async fn load_stream(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<StreamState> {
        if let Some(stream) = self.cache.get(&stream_id.to_string()) {
            return Ok(stream.clone());
        }

        let stream = self.loader.load_stream(ceramic, stream_id, tip).await?;
        // TODO: insert data into cache
        Ok(stream)
    }

    async fn load_streams(
        &self,
        ceramic: &Ceramic,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        self.loader.load_streams(ceramic, account, model_id).await
    }
}
