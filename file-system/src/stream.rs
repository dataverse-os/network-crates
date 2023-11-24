use anyhow::Context;
use dataverse_ceramic::stream::EventsPublisher;
use dataverse_iroh_store::Stream;
use dataverse_types::{
    ceramic::{StreamId, StreamState},
    store::dapp::ModelStore,
};

pub trait StreamOperator: StreamLoader + StreamPublisher {}

#[async_trait::async_trait]
impl StreamOperator for dataverse_iroh_store::Client {}

#[async_trait::async_trait]
impl StreamOperator for () {}

#[async_trait::async_trait]
pub trait StreamLoader {
    async fn load_stream(
        &self,
        ceramic: &String,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState>;

    async fn load_streams(
        &self,
        account: &Option<String>,
        ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>>;
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
impl<T: StreamLoader + Send + Sync> StreamLoader for CachedStreamLoader<T> {
    async fn load_stream(
        &self,
        ceramic: &String,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        if let Some(stream) = self.cache.get(&stream_id.to_string()) {
            return Ok(stream.clone());
        }

        let stream = self.loader.load_stream(ceramic, stream_id).await?;
        // TODO: insert data into cache
        Ok(stream)
    }

    async fn load_streams(
        &self,
        account: &Option<String>,
        ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        self.loader.load_streams(account, ceramic, model_id).await
    }
}

#[async_trait::async_trait]
impl StreamLoader for dataverse_iroh_store::Client {
    async fn load_stream(
        &self,
        _ceramic: &String,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let stream = self.load_stream(stream_id).await?;
        self.load_stream_state(&stream).await
    }

    async fn load_streams(
        &self,
        account: &Option<String>,
        _ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let states = self.list_stream_states_in_model(model_id).await?;
        if let Some(account) = account {
            let mut streams = Vec::new();
            for state in states {
                if state.controllers().contains(&account) {
                    streams.push(state);
                }
            }
            return Ok(streams);
        } else {
            return Ok(states);
        }
    }
}

#[async_trait::async_trait]
impl StreamLoader for () {
    async fn load_stream(
        &self,
        ceramic: &String,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let ceramic = dataverse_ceramic::http::Client::init(ceramic)?;
        let stream = ceramic.ceramic.get(stream_id).await?;
        let state = stream.state.context("Failed to load stream")?.try_into()?;
        Ok(state)
    }

    async fn load_streams(
        &self,
        _account: &Option<String>,
        ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let ceramic = dataverse_ceramic::http::Client::init(ceramic)?;
        let edges = ceramic.ceramic.query_all(None, model_id, None).await?;

        let mut streams = Vec::new();
        for edge in edges {
            if let Some(node) = edge.node {
                streams.push(node.try_into()?);
            }
        }
        Ok(streams)
    }
}

#[async_trait::async_trait]
pub trait StreamPublisher {
    async fn publish_all_streams(&self) -> anyhow::Result<()>;
    async fn publish_stream(&self, stream: Stream) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl StreamPublisher for () {
    async fn publish_all_streams(&self) -> anyhow::Result<()> {
        todo!("publish streams");
    }

    async fn publish_stream(&self, _stream: Stream) -> anyhow::Result<()> {
        todo!("publish stream");
    }
}

#[async_trait::async_trait]
impl StreamPublisher for dataverse_iroh_store::Client {
    async fn publish_all_streams(&self) -> anyhow::Result<()> {
        let streams = self.list_all_streams().await?;
        for stream in streams {
            let commits = self.load_commits(&stream.tip).await?;
            if stream.published == commits.len() {
                continue;
            }
            self.publish_stream(stream).await?;
        }
        Ok(())
    }

    async fn publish_stream(&self, mut stream: Stream) -> anyhow::Result<()> {
        let model_store = ModelStore::get_instance();
        let ceramic = model_store.get_dapp_ceramic(&stream.dapp_id).await?;
        let stream_id = stream.stream_id()?;
        let events = self.load_commits(&stream.tip).await?;
        let ceramic = dataverse_ceramic::http::Client::init(&ceramic)?;
        stream.published = events.len();
        ceramic.publish_events(&stream_id, events).await?;

        // self.save_stream(&stream).await?;
        Ok(())
    }
}
