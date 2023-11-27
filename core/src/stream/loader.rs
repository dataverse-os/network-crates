use anyhow::Context;
use ceramic_core::StreamId;
use dataverse_ceramic::StreamState;

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
