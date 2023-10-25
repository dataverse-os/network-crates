use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use ceramic_core::StreamId;
use dataverse_types::ceramic::StreamState;
use futures::TryStreamExt;
use iroh::baomap::flat::Store as BaoFileStore;
pub use iroh::net::key::SecretKey;
use iroh::{
    client::mem::{Doc, Iroh},
    node::Node,
    rpc_protocol::DocTicket,
};
use iroh_bytes::util::runtime;
use iroh_sync::{store::GetFilter, store::Store, AuthorId};
use iroh_sync::{Author, NamespaceId, NamespacePublicKey};
use json_patch::Patch;
use serde::{Deserialize, Serialize};

use crate::commit::Genesis;

pub struct Client {
    pub iroh: Iroh,
    pub author: AuthorId,
    pub streams: Doc,
    pub patch: Doc,
    pub model: Doc,
    pub genesis: Doc,
}

pub struct KeySet {
    pub author: String,

    pub model: String,
    pub streams: String,
    pub patch: String,
    pub genesis: String,
}

impl KeySet {
    pub fn new(author: &str, model: &str, streams: &str, patch: &str, genesis: &str) -> Self {
        Self {
            author: author.to_string(),
            model: model.to_string(),
            streams: streams.to_string(),
            patch: patch.to_string(),
            genesis: genesis.to_string(),
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
            patch: Client::init_store(&client, &key_set.patch).await?,
            model: Client::init_store(&client, &key_set.model).await?,
            genesis: Client::init_store(&client, &key_set.genesis).await?,
            iroh: client,
        })
    }

    async fn init_store(client: &Iroh, key: &str) -> anyhow::Result<Doc> {
        let ticket = DocTicket::new(NamespaceId::from_str(key)?.to_bytes(), vec![]);
        client.docs.import(ticket).await
    }

    async fn new_doc_model(&self, model_id: &StreamId) -> anyhow::Result<Doc> {
        let model = self.iroh.docs.create().await?;
        let key = b"model".to_vec();
        let model_id = model_id.to_string().as_bytes().to_vec();
        model.set_bytes(self.author, key, model_id.clone()).await?;
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
            Some(id) => match self.iroh.docs.get(id).await? {
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

    pub async fn list_stream_in_model(
        &self,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let doc: Doc = self.lookup_model_doc(model_id).await?;
        let mut stream = doc.get_many(GetFilter::All).await?;
        let mut result = Vec::new();
        while let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            let mut state: StreamState = serde_json::from_slice(&content)?;

            if let Some(patch) = self.load_stream_patch(&state.stream_id()?).await? {
                json_patch::patch(&mut state.content, &patch.patch)?;
            }
            result.push(state);
        }
        Ok(result)
    }

    pub async fn load_stream(
        &self,
        model_id: Option<&StreamId>,
        stream_id: &StreamId,
    ) -> anyhow::Result<Option<StreamState>> {
        let model_id = match model_id {
            Some(id) => id.clone(),
            None => match self.get_model_of_stream(stream_id).await {
                Ok(id) => id,
                Err(_) => return Ok(None),
            },
        };

        let doc = self.lookup_model_doc(&model_id).await?;
        let key = stream_id.to_vec()?;
        let mut stream = doc.get_many(GetFilter::Key(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = doc.read_to_bytes(&entry).await?;
            let mut stream: Stream = serde_json::from_slice(&content)?;

            if let Some(patch) = self.load_stream_patch(stream_id).await? {
                json_patch::patch(&mut stream.state.content, &patch.patch)?;
            }

            return Ok(Some(stream.state));
        }
        Ok(None)
    }

    async fn load_stream_patch(&self, stream_id: &StreamId) -> anyhow::Result<Option<StreamPatch>> {
        let key = stream_id.to_vec()?;
        let mut stream = self.patch.get_many(GetFilter::Key(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = self.patch.read_to_bytes(&entry).await?;
            let content: StreamPatch = serde_json::from_slice(&content)?;
            return Ok(Some(content));
        }
        Ok(None)
    }

    pub async fn save_stream_patch(
        &self,
        stream_id: &StreamId,
        patch: StreamPatch,
    ) -> anyhow::Result<()> {
        let key = stream_id.to_vec()?;
        let value = serde_json::to_vec(&patch)?;
        self.patch.set_bytes(self.author, key, value).await?;
        Ok(())
    }

    pub async fn save_genesis_commit(&self, genesis: Genesis) -> anyhow::Result<()> {
        let stream_id = genesis.stream_id()?;
        let key = stream_id.to_vec()?;
        let value = serde_json::to_vec(&genesis)?;
        self.genesis.set_bytes(self.author, key, value).await?;
        Ok(())
    }

    pub async fn save_stream(&self, state: &StreamState) -> anyhow::Result<()> {
        let model_id = state.model()?;
        let stream_id = state.stream_id()?;
        self.set_model_of_stream(&stream_id, &model_id).await?;
        let doc = self.lookup_model_doc(&model_id).await?;
        let key = stream_id.to_vec()?;
        let value = serde_json::to_vec(&Stream::new(state.clone()))?;
        doc.set_bytes(self.author, key, value).await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stream {
    pub state: StreamState,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Stream {
    pub fn new(state: StreamState) -> Self {
        Self {
            state,
            updated_at: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamPatch {
    pub patch: json_patch::Patch,
    pub jws: ceramic_core::Jws,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl StreamPatch {
    pub fn new(patch: json_patch::Patch, jws: ceramic_core::Jws) -> Self {
        Self {
            patch,
            jws,
            updated_at: chrono::Utc::now(),
        }
    }

    pub fn verify(&self) -> anyhow::Result<()> {
        let patch: Patch = serde_json::from_slice(&self.jws.payload.to_vec()?)?;
        if patch != self.patch {
            anyhow::bail!("invalid patch");
        }
        // TODO: verify signature
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use ceramic_core::Base64UrlString;
    use json_patch::Patch;
    use serde_json::{from_value, json};

    use super::*;

    async fn init_client() -> anyhow::Result<Client> {
        let key = SecretKey::from_str("vprfpdhssy5erwum2ql2sijgpr4rpeq4zacwcjrbxfmxmwtgeo3a")?;
        let temp = tempfile::tempdir()?;
        println!("temp dir: {:?}", temp);
        let key_set = KeySet {
            author: "q7eqbabgzwhu6be7xiy67jkajevrawb32cauytinv6aw4szlozka".to_string(),
            model: "lmnjsx6pmazhkr5ixhhtaw365pcengpawe36yhczcw6qrz2xxqzq".to_string(),
            streams: "ckuuo72r7skny5qy6njecmbgbix6ifn5wxg5sakqfvsamjsiohqq".to_string(),
            patch: "jzf3i7hxilbrnhwjp3ujte3xlkysmyvpydck6ilod2gmtlnokr7q".to_string(),
            genesis: "uf73tr2vc35bedfevh3ovsvr2i3p2wq7ywveict4anmmmqyufoaa".to_string(),
        };

        let client = Client::new(temp.into_path(), key, key_set).await?;
        Ok(client)
    }

    #[tokio::test]
    async fn test_load_stream() -> anyhow::Result<()> {
        let client = init_client().await;
        assert!(client.is_ok());
        let client = client.unwrap();

        // test create model
        let model_id =
            StreamId::from_str("kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso")?;
        let model = client.new_doc_model(&model_id).await;
        assert!(model.is_ok());

        // test list models
        let models = client.list_models().await;
        assert!(models.is_ok());

        // update stream state
        let state: StreamState = from_value(json!({
          "type": 3,
          "content": {
            "fileName": "post",
            "fileType": 0,
            "contentId": "",
            "createdAt": "2023-09-06T05:22:50.338Z",
            "fsVersion": "0.11",
            "updatedAt": "2023-09-06T05:22:50.338Z",
            "contentType": "eyJyZXNvdXJjZSI6IkNFUkFNSUMiLCJyZXNvdXJjZUlkIjoia2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1In0"
          },
          "metadata": {
            "controllers": [
              "did:pkh:eip155:1:0x312eA852726E3A9f633A0377c0ea882086d66666"
            ],
            "model": "kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso"
          },
          "signature": 2,
          "anchorStatus": "ANCHORED",
          "log": [
            {
              "cid": "bagcqceraeeto3737ppwcmowjns25bilelzipyxrb4ehjmxz2a3dzbk4llfaq",
              "type": 0,
              "expirationTime": 1694581559,
              "timestamp": 1693989407
            },
            {
              "cid": "bagcqcerad4ksqqygh5wux6ephrnbyppy3ij2tpwxqf2dlsa4mefhkptlvtpa",
              "type": 1,
              "expirationTime": 1694585509,
              "timestamp": 1693989407
            },
            {
              "cid": "bagcqcerakqybffchxpqeqtngbm7n7zk52ypmfml6z5yzop6k6oumyl5nqbfq",
              "type": 1,
              "expirationTime": 1694585578,
              "timestamp": 1693989407
            },
            {
              "cid": "bagcqcerac44ndmh5fn56c7ypcwqlbsvsja7anyyw6cik7gmdvp2e7tmowsrq",
              "type": 1,
              "expirationTime": 1694585628,
              "timestamp": 1693989407
            },
            {
              "cid": "bagcqceraldkbjoyvm6urgva2mec53ukzh4s2sexabzegqen3kkakn6gswnea",
              "type": 1,
              "expirationTime": 1694585645,
              "timestamp": 1693989407
            },
            {
              "cid": "bafyreia5jai7fsmgjpzbeixyn2jari27ymnmkydm5fvwwkm4otthvbohty",
              "type": 2,
              "timestamp": 1693989407
            }
          ],
          "anchorProof": {
            "root": "bafyreiaxfjkme33rujt5wfajbl7r6pcdhjw4gfzwmxqe7xs4wf3dwvxdpy",
            "txHash": "bagjqcgzasq3bv55stn7sg6m6zhmfq2fhsdgt4sef4fwozianarbmemjmhu6q",
            "txType": "f(bytes32)",
            "chainId": "eip155:1"
          },
          "doctype": "MID"
        }))?;

        let save_res = client.save_stream(&state).await;
        assert!(save_res.is_ok());

        // load origin stream
        let stream = client
            .load_stream(Some(&model_id), &state.stream_id().unwrap())
            .await;

        assert!(stream.is_ok());

        // TODO: verify jws content and signature
        let empty_jws = ceramic_core::Jws {
            link: None,
            payload: Base64UrlString::from(vec![]),
            signatures: vec![],
        };
        let patch: Patch = from_value(json!([
            { "op": "replace", "path": "/fileName", "value": "post2" },
        ]))
        .unwrap();

        let patch = StreamPatch::new(patch, empty_jws);

        let res = client
            .save_stream_patch(&state.stream_id().unwrap(), patch)
            .await;
        assert!(res.is_ok());

        let stream = client
            .load_stream(Some(&model_id), &state.stream_id().unwrap())
            .await;
        assert!(stream.is_ok());
        let stream = stream.unwrap();
        assert!(stream.is_some());
        let stream = stream.unwrap();
        assert_eq!(stream.content["fileName"], "post2");

        Ok(())
    }
}
