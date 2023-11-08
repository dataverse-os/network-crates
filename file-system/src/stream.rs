use anyhow::{Context, Ok};
use dataverse_iroh_store::commit::{Data, Genesis};
use dataverse_types::{
    ceramic::{LogType, StreamId, StreamState},
    store::dapp::ModelStore,
};

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
        stream.try_into()
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

#[async_trait::async_trait]
pub trait StreamPublisher {
    async fn publish_streams(&self) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl StreamPublisher for dataverse_iroh_store::Client {
    async fn publish_streams(&self) -> anyhow::Result<()> {
        let streams = self.list_all_streams().await?;
        let model_store = ModelStore::get_instance();
        for stream in streams {
            if stream.published == stream.commits.len() {
                continue;
            }
            let ceramic = model_store.get_dapp_ceramic(&stream.dapp_id).await?;
            let client = reqwest::Client::new();
            let stream_id = stream.stream_id()?;
            for ele in stream.commits {
                match ele.log_type() {
                    LogType::Genesis => {
                        let url = format!("{}/api/v0/streams", ceramic);
                        let genesis = Genesis {
                            r#type: stream.r#type,
                            genesis: ele.try_into()?,
                            opts: serde_json::Value::Null,
                        };
                        let res = client.post(&url).json(&genesis).send().await?;
                        log::debug!("publish genesis: {:?}", res)
                    }
                    LogType::Signed => {
                        let url = format!("{}/api/v0/commits", ceramic);
                        let genesis = Data {
                            stream_id: stream_id.clone(),
                            commit: ele.try_into()?,
                            opts: serde_json::Value::Null,
                        };
                        let res = client.post(&url).json(&genesis).send().await?;
                        log::debug!("publish signed: {:?}", res)
                    }
                    _ => anyhow::bail!("invalid log type"),
                };
            }
            todo!("modify stream save new published");
        }
        todo!("publish streams");
    }
}
