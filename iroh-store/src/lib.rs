pub mod file;

pub use file::*;

use std::sync::Arc;
use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use ceramic_core::{Cid, StreamId};
use dataverse_ceramic::stream::StreamState;
use dataverse_ceramic::{Ceramic, StreamLoader, StreamOperator, StreamsLoader};
use dataverse_core::stream::{Stream, StreamStore};
use futures::TryStreamExt;
use iroh::client::mem::{Doc, Iroh};
pub use iroh::net::key::SecretKey;
use iroh::node::Node;
use iroh::rpc_protocol::DocTicket;
use iroh_bytes::{store::flat::Store as BaoFileStore, util::runtime};
use iroh_sync::store::{Query, Store};
use iroh_sync::{Author, AuthorId, NamespaceId, NamespacePublicKey, NamespaceSecret};

pub struct Client {
    pub iroh: Iroh,
    pub operator: Arc<dyn StreamOperator>,
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
    pub async fn new(
        data_path: PathBuf,
        key: SecretKey,
        key_set: KeySet,
        operator: Arc<dyn StreamOperator + Send + Sync>,
    ) -> anyhow::Result<Self> {
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
            operator,
        })
    }

    async fn init_store(client: &Iroh, key: &str) -> anyhow::Result<Doc> {
        let ticket = DocTicket::new(
            iroh_sync::Capability::Write(NamespaceSecret::from_str(key)?),
            vec![],
        );
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
        let mut stream = self.streams.get_many(Query::all()).await?;
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
        let mut stream = self.streams.get_many(Query::all()).await?;
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
        let mut stream = self.model.get_many(Query::key_exact(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = self.model.read_to_bytes(&entry).await?;
            let content: StreamId = StreamId::try_from(content.to_vec().as_slice())?;
            return Ok(content);
        }
        anyhow::bail!("model of stream `{}` not found", stream_id)
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
        let mut stream = doc.get_many(Query::all()).await?;
        let mut result = Vec::new();
        while let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            result.push(serde_json::from_slice(&content)?);
        }
        Ok(result)
    }
}

#[async_trait::async_trait]
impl StreamStore for Client {
    async fn save_stream(&self, stream: &Stream) -> anyhow::Result<()> {
        let stream_id = stream.stream_id()?;
        let key = stream_id.to_vec()?;
        let value = serde_json::to_vec(&stream)?;

        match &stream.model {
            Some(model) => {
                self.set_model_of_stream(&stream_id, &model).await?;
                self.lookup_model_doc(&model)
                    .await?
                    .set_bytes(self.author, key, value)
                    .await?;
            }
            _ => todo!("save stream without model"),
        }
        Ok(())
    }

    async fn load_stream(&self, stream_id: &StreamId) -> anyhow::Result<Option<Stream>> {
        let model_id = self.get_model_of_stream(stream_id).await?;
        let key = stream_id.to_vec()?;

        let doc = &self.lookup_model_doc(&model_id).await?;
        let mut stream = doc.get_many(Query::key_exact(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            let content: Stream = serde_json::from_slice(&content)?;
            return Ok(Some(content));
        }
        log::warn!(
            "looking for stream `{}`: not found in model `{}`",
            stream_id,
            model_id
        );
        Ok(None)
    }
}

#[async_trait::async_trait]
impl StreamsLoader for Client {
    async fn load_stream_states(
        &self,
        ceramic: &Ceramic,
        _account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let mut result = Vec::new();
        let streams = self.list_stream_in_model(model_id).await?;
        for stream in streams {
            let (stream_id, tip) = (stream.stream_id()?, Some(stream.tip));
            let state = self
                .operator
                .load_stream_state(ceramic, &stream_id, tip)
                .await?;
            result.push(state);
        }
        Ok(result)
    }
}

#[async_trait::async_trait]
impl StreamLoader for Client {
    async fn load_stream_state(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<StreamState> {
        let tip = match tip {
            Some(tip) => tip,
            None => {
                self.load_stream(stream_id)
                    .await?
                    .context(format!("stream not found: {}", stream_id))?
                    .tip
            }
        };

        self.operator
            .load_stream_state(ceramic, stream_id, Some(tip))
            .await
    }
}

#[cfg(test)]
mod tests {
    use dataverse_ceramic::{event::Event, kubo};

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
        let kubo_path = "http://localhost:5001";
        let kubo = kubo::new(kubo_path);
        let kubo = Arc::new(kubo);

        let client = Client::new(temp.into_path(), key, key_set, kubo).await?;
        Ok(client)
    }

    #[tokio::test]
    async fn operate_stream() -> anyhow::Result<()> {
        let client = init_client().await;
        assert!(client.is_ok());
        let client = client.unwrap();

        let genesis = dataverse_ceramic::commit::example::genesis();

        println!(
            "extract stream_id from genesis: {:?}",
            genesis.stream_id()?.to_string()
        );

        // save genesis commit
        let dapp_id = uuid::Uuid::new_v4();
        let commit: Event = genesis.genesis.try_into().unwrap();
        let mut commits = vec![commit.clone()];
        let state = StreamState::new(genesis.r#type, commits.clone());
        assert!(state.is_ok());
        let state = state.unwrap();
        let mut stream =
            Stream::new(&dapp_id, genesis.r#type, &commit, state.model().ok()).unwrap();
        let res = client.save_stream(&stream).await;
        assert!(res.is_ok());
        let update_at = state.content["updatedAt"].clone();

        // save data commit
        let data = dataverse_ceramic::commit::example::data();
        let commit: Event = data.commit.try_into().unwrap();
        stream.tip = commit.cid;
        commits.push(commit);
        let res = client.save_stream(&stream).await;
        assert!(res.is_ok());

        // load stream
        let stream_id = stream.stream_id()?;
        let stream = client.load_stream(&stream_id).await;
        assert!(stream.is_ok());
        let stream = stream.unwrap();
        assert!(stream.is_some());

        // load commits
        let state = stream.unwrap().state(commits);
        assert!(state.is_ok());
        let state = state.unwrap();
        let update_at_mod = state.content["updatedAt"].clone();
        assert_ne!(update_at, update_at_mod);

        // list stream state in model
        let streams = client.list_stream_in_model(&state.model()?).await;
        assert!(streams.is_ok());
        assert_eq!(streams.unwrap().len(), 1);
        Ok(())
    }
}
