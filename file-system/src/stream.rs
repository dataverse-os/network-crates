use anyhow::{Context, Ok};
use dataverse_types::ceramic::{StreamId, StreamState};

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
        let stream = self.load_stream2(stream_id).await?;
        stream.try_into()
    }

    async fn load_streams(
        &self,
        _account: &Option<String>,
        _ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        self.load_streams2(model_id).await
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
        stream.state.context("Failed to load stream")
    }

    async fn load_streams(
        &self,
        _account: &Option<String>,
        ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let ceramic = dataverse_ceramic::http::Client::init(ceramic)?;
        let edges = ceramic.ceramic.query_all(&None, model_id, None).await?;

        let mut streams = Vec::new();
        for edge in edges {
            if let Some(node) = edge.node {
                streams.push(node);
            }
        }
        Ok(streams)
    }
}
