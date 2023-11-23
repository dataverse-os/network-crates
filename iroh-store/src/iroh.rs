use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use ceramic_core::Cid;
use ceramic_kubo_rpc_server::models::Codecs::{DagCbor, DagJose};
use ceramic_kubo_rpc_server::models::{self};
use ceramic_kubo_rpc_server::BlockGetPostResponse;
use chrono::{DateTime, Utc};
use dataverse_ceramic::commit::{Content, Data, Genesis};
use dataverse_ceramic::event::{self, VerifyOption};
use dataverse_ceramic::jws::ToCid;
use dataverse_ceramic::kubo;
use dataverse_types::ceramic::{StreamId, StreamState};
use futures::TryStreamExt;
use iroh::client::mem::{Doc, Iroh};
pub use iroh::net::key::SecretKey;
use iroh::node::Node;
use iroh::rpc_protocol::DocTicket;
use iroh_bytes::{store::flat::Store as BaoFileStore, util::runtime};
use iroh_sync::store::{Query, Store};
use iroh_sync::{Author, AuthorId, NamespaceId, NamespacePublicKey, NamespaceSecret};
use serde::{Deserialize, Serialize};
use swagger::ByteArray;

pub struct Client {
    pub iroh: Iroh,
    pub kubo: kubo::Client,
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
        kubo_path: String,
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
            kubo: kubo::new(&kubo_path),
        })
    }

    pub async fn load_cid(&self, cid: &Cid) -> anyhow::Result<Vec<u8>> {
        let result;
        let res = self
            .kubo
            .block_get_post(cid.to_string(), Some("1s".into()), None)
            .await?;
        match res {
            BlockGetPostResponse::Success(bytes) => {
                result = bytes.to_vec();
            }
            _ => anyhow::bail!("cid not found with in ipfs: {}", cid),
        }

        Ok(result)
    }

    pub async fn load_commits(&self, tip: &Cid) -> anyhow::Result<Vec<event::Event>> {
        let mut commits = Vec::new();
        let mut cid = tip.clone();
        loop {
            let bytes = self.load_cid(&cid).await?;
            let mut commit = event::Event::decode(cid, bytes.to_vec())?;
            match &mut commit.value {
                event::EventValue::Signed(signed) => {
                    signed.linked_block = Some(self.load_cid(&signed.payload_link()?).await?);
                    signed.cacao_block = Some(self.load_cid(&signed.cap()?).await?);
                }
                event::EventValue::Anchor(anchor) => {
                    anchor.proof_block = Some(self.load_cid(&anchor.proof).await?)
                }
            }
            commits.insert(0, commit.clone());
            match commit.prev()? {
                Some(prev) => cid = prev,
                None => break,
            };
        }
        Ok(commits)
    }

    pub async fn load_stream_state(&self, stream: &Stream) -> anyhow::Result<StreamState> {
        stream.to_state(self.load_commits(&stream.tip).await?)
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

    pub async fn list_stream_states_in_model(
        &self,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let mut result = Vec::new();
        let streams = self.list_stream_in_model(model_id).await?;
        for stream in streams {
            let state = self.load_stream_state(&stream).await?;
            result.push(state);
        }
        Ok(result)
    }

    pub async fn save_jws_blobs(&self, content: &Content) -> anyhow::Result<()> {
        let kubo = &self.kubo;
        let mhtype = Some(models::Multihash::Sha2256);
        let _ = kubo
            .block_put_post(
                ByteArray(content.linked_block.to_vec()?),
                Some(DagCbor),
                mhtype,
                Some(true),
            )
            .await?;
        let _ = kubo
            .block_put_post(
                ByteArray(content.cacao_block.to_vec()?),
                Some(DagCbor),
                mhtype,
                Some(true),
            )
            .await?;
        let _ = kubo
            .block_put_post(
                ByteArray(content.jws.to_vec()?),
                Some(DagJose),
                mhtype,
                Some(true),
            )
            .await?;
        Ok(())
    }

    pub async fn save_genesis_commit(
        &self,
        dapp_id: &uuid::Uuid,
        genesis: Genesis,
    ) -> anyhow::Result<(Stream, StreamState)> {
        let stream_id = genesis.stream_id()?;
        self.save_jws_blobs(&genesis.genesis).await?;
        let commit: event::Event = genesis.genesis.try_into()?;
        // check if commit already exists
        if let Ok(stream) = self.load_stream(&stream_id).await {
            let commits = &self.load_commits(&stream.tip).await?;
            if commits.iter().any(|ele| ele.cid == commit.cid) {
                let state = stream.to_state(commits.to_vec())?;
                return Ok((stream, state));
            }
        }
        let stream = Stream::new(dapp_id.clone(), genesis.r#type, &commit)?;
        let state = stream.to_state(vec![commit.clone()])?;
        self.save_stream(&state.model()?, &stream).await?;
        let opts = vec![
            VerifyOption::ResourceModelsContain(state.model()?),
            VerifyOption::ExpirationTimeBefore(Utc::now() - chrono::Duration::days(100)),
        ];
        commit.verify_signature(opts)?;
        Ok((stream, state))
    }

    pub async fn save_data_commit(
        &self,
        _dapp_id: &uuid::Uuid,
        data: Data,
    ) -> anyhow::Result<(Stream, StreamState)> {
        let mut stream = self
            .load_stream(&data.stream_id)
            .await
            .context("stream not exist")?;
        self.save_jws_blobs(&data.commit).await?;
        let commit: event::Event = data.commit.try_into()?;
        // check if commit already exists
        let commits = &self.load_commits(&stream.tip).await?;
        if commits.iter().any(|ele| ele.cid == commit.cid) {
            let state = stream.to_state(commits.to_vec())?;
            return Ok((stream, state));
        }
        let prev = commit.prev()?.context("prev commit not found")?;
        if commits.iter().all(|ele| ele.cid != prev) {
            anyhow::bail!("donot have prev commit");
        }
        let state = stream.to_state(commits.to_vec())?;

        stream.tip = commit.cid;
        let model = state.model()?;
        self.save_stream(&model, &stream).await?;
        let opts = vec![
            VerifyOption::ResourceModelsContain(model),
            VerifyOption::ExpirationTimeBefore(Utc::now() - chrono::Duration::days(100)),
        ];
        commit.verify_signature(opts)?;
        Ok((stream, state))
    }

    pub async fn save_stream(&self, model: &StreamId, stream: &Stream) -> anyhow::Result<()> {
        let stream_id = stream.stream_id()?;
        let key = stream_id.to_vec()?;
        let value = serde_json::to_vec(&stream)?;

        self.set_model_of_stream(&stream_id, &model).await?;
        self.lookup_model_doc(&model)
            .await?
            .set_bytes(self.author, key, value)
            .await?;
        Ok(())
    }

    pub async fn load_stream(&self, stream_id: &StreamId) -> anyhow::Result<Stream> {
        let model_id = self.get_model_of_stream(stream_id).await?;
        let key = stream_id.to_vec()?;

        let doc = &self.lookup_model_doc(&model_id).await?;
        let mut stream = doc.get_many(Query::key_exact(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            let content: Stream = serde_json::from_slice(&content)?;
            return Ok(content);
        }
        anyhow::bail!("stream `{}` not found in model `{}`", stream_id, model_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub r#type: u64,
    pub dapp_id: uuid::Uuid,
    pub expiration_time: Option<DateTime<Utc>>,
    pub genesis: Cid,
    pub tip: Cid,
    pub published: usize,
}

impl Stream {
    pub fn new(dapp_id: uuid::Uuid, r#type: u64, commit: &event::Event) -> anyhow::Result<Self> {
        if let event::EventValue::Signed(signed) = &commit.value {
            let expiration_time = match signed.cacao()? {
                Some(cacao) => cacao.p.expiration_time()?,
                None => None,
            };
            return Ok(Stream {
                r#type,
                dapp_id,
                expiration_time,
                published: 0,
                tip: commit.cid,
                genesis: commit.cid,
            });
        }
        anyhow::bail!("invalid genesis commit");
    }

    pub fn to_state(&self, commits: Vec<event::Event>) -> anyhow::Result<StreamState> {
        let mut state = StreamState {
            r#type: self.r#type,
            ..Default::default()
        };
        for ele in commits {
            ele.apply_to(&mut state)?;
        }
        Ok(state)
    }

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        Ok(StreamId {
            r#type: self.r#type.try_into()?,
            cid: self.genesis,
        })
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
        let kubo_path = "http://localhost:5001";

        let client = Client::new(temp.into_path(), key, key_set, kubo_path.into()).await?;
        Ok(client)
    }

    #[tokio::test]
    async fn operate_stream() -> anyhow::Result<()> {
        let client = init_client().await;
        assert!(client.is_ok());
        let client = client.unwrap();

        let genesis: Genesis = dataverse_ceramic::commit::example::genesis();

        println!(
            "extract stream_id from genesis: {:?}",
            genesis.stream_id()?.to_string()
        );

        // save genesis commit
        let dapp_id = uuid::Uuid::new_v4();
        let state = client.save_genesis_commit(&dapp_id, genesis).await;
        assert!(state.is_ok());
        let (_, state) = state.unwrap();
        let update_at = state.content["updatedAt"].clone();

        // save data commit
        let data: Data = dataverse_ceramic::commit::example::data();
        let result = client.save_data_commit(&dapp_id, data).await;
        assert!(result.is_ok());
        let stream_id = state.stream_id()?;

        // load stream
        let stream = client.load_stream(&stream_id).await;
        assert!(stream.is_ok());
        let stream = stream.unwrap();

        // load commits
        let commits = client.load_commits(&stream.tip).await?;
        assert_eq!(commits.len(), 2);
        let state: anyhow::Result<StreamState> = stream.to_state(commits);
        assert!(state.is_ok());
        let state = state.unwrap();
        let update_at_mod = state.content["updatedAt"].clone();
        assert_ne!(update_at, update_at_mod);

        // list stream state in model
        let streams = client.list_stream_states_in_model(&state.model()?).await;
        assert!(streams.is_ok());
        assert_eq!(streams.unwrap().len(), 1);
        Ok(())
    }
}
