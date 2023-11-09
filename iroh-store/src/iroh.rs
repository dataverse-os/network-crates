use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use chrono::{DateTime, Utc};
use dataverse_ceramic::event;
use dataverse_types::ceramic::{StreamId, StreamState};
use futures::TryStreamExt;
pub use iroh::net::key::SecretKey;
use iroh::{
    client::mem::{Doc, Iroh},
    node::Node,
    rpc_protocol::DocTicket,
};
use iroh_bytes::store::flat::Store as BaoFileStore;
use iroh_bytes::util::runtime;
use iroh_sync::{store::GetFilter, store::Store, AuthorId};
use iroh_sync::{Author, NamespaceId, NamespacePublicKey};
use serde::{Deserialize, Serialize};

use crate::commit::{Data, Genesis};

pub struct Client {
    pub iroh: Iroh,
    pub author: AuthorId,
    pub streams: Doc,
    pub model: Doc,
}

pub struct KeySet {
    pub author: String,

    pub model: String,
    pub streams: String,
}

impl KeySet {
    pub fn new(author: &str, model: &str, streams: &str) -> Self {
        Self {
            author: author.to_string(),
            model: model.to_string(),
            streams: streams.to_string(),
        }
    }
}

pub const DEFAULT_RPC_PORT: u16 = 0x1337;

impl Client {
    pub async fn new(data_path: PathBuf, key: SecretKey, key_set: KeySet) -> anyhow::Result<Self> {
        let rt = runtime::Handle::from_current(num_cpus::get())?;

        let bao_path = data_path.join("iroh/bao");
        let bao_store = BaoFileStore::load(&bao_path, &bao_path, &bao_path, &rt)
            .await
            .with_context(|| {
                format!("Failed to load tasks database from {}", data_path.display())
            })?;

        let path = data_path.join("iroh/docs.redb");
        let doc_store = iroh_sync::store::fs::Store::new(path)?;

        let author: Author = Author::from_str(&key_set.author)?;
        doc_store.import_author(author.clone())?;

        let node = Node::builder(bao_store, doc_store)
            .runtime(&rt)
            .secret_key(key)
            .spawn()
            .await?;
        let client: Iroh = node.client();

        Ok(Self {
            author: author.id(),
            streams: Client::init_store(&client, &key_set.streams).await?,
            model: Client::init_store(&client, &key_set.model).await?,
            iroh: client,
        })
    }

    async fn init_store(client: &Iroh, key: &str) -> anyhow::Result<Doc> {
        let ticket = DocTicket::new(NamespaceId::from_str(key)?.to_bytes(), vec![]);
        client.docs.import(ticket).await
    }

    async fn new_doc_model(&self, model_id: &StreamId) -> anyhow::Result<Doc> {
        let model = self.iroh.docs.create().await?;
        let model_id = model_id.to_string().as_bytes().to_vec();
        let namespace_id = model.id().as_bytes().to_vec();
        self.streams
            .set_bytes(self.author, model_id, namespace_id)
            .await?;
        Ok(model)
    }

    pub async fn list_models(&self) -> anyhow::Result<Vec<StreamId>> {
        let mut stream = self.streams.get_many(GetFilter::All).await?;
        let mut result = Vec::new();
        while let Some(entry) = stream.try_next().await? {
            let str = String::from_utf8(entry.key().to_vec())?;
            result.push(StreamId::from_str(&str)?);
        }
        Ok(result)
    }

    pub async fn list_all_streams(&self) -> anyhow::Result<Vec<Stream>> {
        let mut result = Vec::new();
        let models = self.list_models().await?;
        for model in models {
            let streams = self.list_stream_in_model(&model).await?;
            for stream in streams {
                result.push(stream);
            }
        }
        Ok(result)
    }

    async fn get_namespace_id_by_model_id(
        &self,
        model_id: &StreamId,
    ) -> anyhow::Result<Option<NamespaceId>> {
        let mut stream = self.streams.get_many(GetFilter::All).await?;
        while let Some(entry) = stream.try_next().await? {
            if entry.key() == model_id.to_string().as_bytes().to_vec() {
                let content = self.streams.read_to_bytes(&entry).await?;
                let key = NamespacePublicKey::from_bytes(content.as_ref().try_into()?)?;
                return Ok(Some(NamespaceId::from(key)));
            }
        }
        Ok(None)
    }

    async fn lookup_model_doc(&self, model_id: &StreamId) -> anyhow::Result<Doc> {
        let id = self.get_namespace_id_by_model_id(model_id).await?;
        match id {
            Some(id) => match self.iroh.docs.open(id).await? {
                Some(doc) => Ok(doc),
                None => Ok(self.new_doc_model(model_id).await?),
            },
            None => Ok(self.new_doc_model(model_id).await?),
        }
    }

    async fn get_model_of_stream(&self, stream_id: &StreamId) -> anyhow::Result<StreamId> {
        let key = stream_id.to_vec()?;
        let mut stream = self.model.get_many(GetFilter::Key(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = self.model.read_to_bytes(&entry).await?;
            let content: StreamId = StreamId::try_from(content.to_vec().as_slice())?;
            return Ok(content);
        }
        anyhow::bail!("not found")
    }

    async fn set_model_of_stream(
        &self,
        stream_id: &StreamId,
        model_id: &StreamId,
    ) -> anyhow::Result<()> {
        let key = stream_id.to_vec()?;
        let value = model_id.to_vec()?;
        self.model.set_bytes(self.author, key, value).await?;
        Ok(())
    }

    async fn list_stream_in_model(&self, model_id: &StreamId) -> anyhow::Result<Vec<Stream>> {
        let doc: Doc = self.lookup_model_doc(model_id).await?;
        let mut stream = doc.get_many(GetFilter::All).await?;
        let mut result = Vec::new();
        while let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            result.push(serde_json::from_slice(&content)?);
        }
        Ok(result)
    }

    pub async fn list_stream_states_in_model(
        &self,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let mut result = Vec::new();
        let streams = self.list_stream_in_model(model_id).await?;
        for stream in streams {
            result.push(stream.try_into()?);
        }
        Ok(result)
    }

    pub async fn save_genesis_commit(
        &self,
        dapp_id: &uuid::Uuid,
        genesis: Genesis,
    ) -> anyhow::Result<StreamState> {
        let stream = self.load_stream(&genesis.stream_id()?).await?;
        let commit: event::Event = genesis.genesis.try_into()?;
        // check if commit already exists
        if stream.commits.iter().any(|ele| ele.cid == commit.cid) {
            return Ok(stream.try_into()?);
        }
        let stream = Stream::new(dapp_id.clone(), genesis.r#type, commit)?;
        self.save_stream(stream).await
    }

    pub async fn save_data_commit(
        &self,
        _dapp_id: &uuid::Uuid,
        data: Data,
    ) -> anyhow::Result<StreamState> {
        let mut stream = self.load_stream(&data.stream_id).await?;
        let commit: event::Event = data.commit.try_into()?;
        // check if commit already exists
        if stream.commits.iter().any(|ele| ele.cid == commit.cid) {
            return Ok(stream.try_into()?);
        }
        stream.add_commit(commit)?;
        self.save_stream(stream).await
    }

    pub async fn save_stream(&self, stream: Stream) -> anyhow::Result<StreamState> {
        let stream_id = stream.stream_id()?;

        let key = stream.stream_id()?.to_vec()?;
        let value = serde_json::to_vec(&stream)?;

        let state: StreamState = stream.try_into()?;
        let model_id = state.model()?;

        self.set_model_of_stream(&stream_id, &model_id).await?;
        self.lookup_model_doc(&model_id)
            .await?
            .set_bytes(self.author, key, value)
            .await?;
        Ok(state)
    }

    pub async fn load_stream(&self, stream_id: &StreamId) -> anyhow::Result<Stream> {
        let model_id = self.get_model_of_stream(stream_id).await?;
        let key = stream_id.to_vec()?;

        let doc = &self.lookup_model_doc(&model_id).await?;
        let mut stream = doc.get_many(GetFilter::Key(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            let content: Stream = serde_json::from_slice(&content)?;
            return Ok(content);
        }
        anyhow::bail!("not found")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stream {
    pub r#type: u64,
    pub dapp_id: uuid::Uuid,
    pub expiration_time: Option<DateTime<Utc>>,
    pub commits: Vec<event::Event>,
    pub published: usize,
}

impl Stream {
    pub fn new(dapp_id: uuid::Uuid, r#type: u64, commit: event::Event) -> anyhow::Result<Self> {
        if let event::EventValue::Signed(signed) = &commit.value {
            let expiration_time = match signed.cacao()? {
                Some(cacao) => {
                    let expiration_time = cacao.p.expiration_time()?;
                    if let Some(exp) = expiration_time {
                        if exp < Utc::now() {
                            anyhow::bail!("genesis commit expired");
                        }
                    }
                    expiration_time
                }
                None => None,
            };

            // TODO: check stream model in cacao resource models
            return Ok(Stream {
                r#type,
                dapp_id,
                expiration_time,
                commits: vec![commit],
                published: 0,
            });
        }
        anyhow::bail!("invalid genesis commit");
    }

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        Ok(StreamId {
            r#type: self.r#type.try_into()?,
            cid: self
                .commits
                .first()
                .context("commits is empty")?
                .cid
                .clone(),
        })
    }

    pub fn add_commit(&mut self, commit: event::Event) -> anyhow::Result<()> {
        let prev = commit.prev()?.context("prev commit not found")?;
        if prev != self.commits.last().context("commits is empty")?.cid {
            anyhow::bail!("prev commit not match");
        }
        if let event::EventValue::Signed(signed) = &commit.value {
            if let Some(cacao) = signed.cacao()? {
                let expiration_time = cacao.p.expiration_time()?;
                if let Some(exp) = expiration_time {
                    if exp < Utc::now() {
                        anyhow::bail!("data commit expired");
                    }
                    // override expiration time if it is earlier
                    if let Some(stream_exp) = self.expiration_time {
                        if exp < stream_exp {
                            self.expiration_time = Some(exp);
                        }
                    }
                }
            };
        }
        self.commits.push(commit);
        Ok(())
    }
}

impl TryInto<StreamState> for Stream {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<StreamState, Self::Error> {
        let mut state = StreamState {
            r#type: self.r#type,
            ..Default::default()
        };

        for ele in self.commits {
            ele.apply_to(&mut state)?;
        }

        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn init_client() -> anyhow::Result<Client> {
        let key = SecretKey::from_str("vprfpdhssy5erwum2ql2sijgpr4rpeq4zacwcjrbxfmxmwtgeo3a")?;
        let temp = tempfile::tempdir()?;
        println!("temp dir: {:?}", temp);
        let key_set = KeySet {
            author: "q7eqbabgzwhu6be7xiy67jkajevrawb32cauytinv6aw4szlozka".to_string(),
            model: "lmnjsx6pmazhkr5ixhhtaw365pcengpawe36yhczcw6qrz2xxqzq".to_string(),
            streams: "ckuuo72r7skny5qy6njecmbgbix6ifn5wxg5sakqfvsamjsiohqq".to_string(),
        };

        let client = Client::new(temp.into_path(), key, key_set).await?;
        Ok(client)
    }

    #[tokio::test]
    async fn test_load_stream() -> anyhow::Result<()> {
        let client = init_client().await;
        assert!(client.is_ok());
        let client = client.unwrap();

        let genesis: Genesis = crate::commit::example::genesis();

        println!(
            "extract stream_id from genesis: {:?}",
            genesis.stream_id()?.to_string()
        );

        let dapp_id = uuid::Uuid::new_v4();
        let state = client.save_genesis_commit(&dapp_id, genesis).await;
        assert!(state.is_ok());
        let state = state.unwrap();
        let update_at = state.content["updatedAt"].clone();

        let data: Data = crate::commit::example::data();

        let result = client.save_data_commit(&dapp_id, data).await;
        assert!(result.is_ok());

        let stream_id = state.stream_id()?;

        let stream = client.load_stream(&stream_id).await;
        assert!(stream.is_ok());
        let stream = stream.unwrap();
        assert_eq!(stream.commits.len(), 2);

        let state: anyhow::Result<StreamState> = stream.try_into();
        assert!(state.is_ok());
        let state = state.unwrap();
        let update_at_mod = state.content["updatedAt"].clone();
        assert_ne!(update_at, update_at_mod);

        let streams = client.list_stream_states_in_model(&state.model()?).await;
        assert!(streams.is_ok());
        assert_eq!(streams.unwrap().len(), 1);
        Ok(())
    }
}
