use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use ceramic_core::StreamId;
use dataverse_types::ceramic::{self, StreamState};
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

use crate::commit::{Data, Genesis};

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
        let stream = Stream2 {
            r#type: genesis.r#type,
            commits: vec![genesis.genesis.try_into()?],
        };
        self.save_stream2(stream).await
    }

    pub async fn save_stream2(&self, stream: Stream2) -> anyhow::Result<()> {
        let key = stream.stream_id()?.to_vec()?;
        let value = serde_json::to_vec(&stream)?;
        self.genesis.set_bytes(self.author, key, value).await?;
        Ok(())
    }

    pub async fn load_streams2(&self, stream_id: &StreamId) -> anyhow::Result<Stream2> {
        let key = stream_id.to_vec()?;
        let mut stream = self.genesis.get_many(GetFilter::Key(key)).await?;
        if let Some(entry) = stream.try_next().await? {
            let content = self.genesis.read_to_bytes(&entry).await?;
            let content: Stream2 = serde_json::from_slice(&content)?;
            return Ok(content);
        }
        anyhow::bail!("not found")
    }

    pub async fn save_data_commit(&self, data: Data) -> anyhow::Result<()> {
        let mut stream = self.load_streams2(&data.stream_id).await?;
        stream.commits.push(data.commit.jws.try_into()?);
        self.save_stream2(stream).await
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
pub struct Stream2 {
    pub r#type: u64,
    pub commits: Vec<ceramic::event::Event>,
}

impl Stream2 {
    fn stream_id(&self) -> anyhow::Result<StreamId> {
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
}

impl TryInto<StreamState> for Stream2 {
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
    async fn test_load_stream2() -> anyhow::Result<()> {
        let client = init_client().await;
        assert!(client.is_ok());
        let client = client.unwrap();

        let genesis: Genesis = serde_json::from_value(serde_json::json!({
            "type": 3,
            "genesis": {
                "jws": {
                    "payload": "AXESIAGefBcDGnG7RXH57wnF-gdDxHHdoEC5KTaZW5GaxBJ0",
                    "signatures": [
                        {
                            "protected": "eyJhbGciOiJFZERTQSIsImNhcCI6ImlwZnM6Ly9iYWZ5cmVpY3EzczJydmlzbGsycnRxajdqZTd4amlpYmNqN2ZjNmd4bHNtNGhmeGFzN3BnNmV4YzZ5bSIsImtpZCI6ImRpZDprZXk6ejZNa3REVkRVaEVhdUxiRUVaTVNBdFIxNzdkRHljZG96Y3hSZndQcVQyalFWSlU3I3o2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVNyJ9",
                            "signature": "E_UPzqbEZVFq55WL-zsmEzSMrh7cRblev1CUJOIRIuk-sxy_g51tgD8tHVfZdZvsrNcdiYDEHF4-EM2qEj6oCw"
                        }
                    ],
                    "link": "bafyreiabtz6boay2og5uk4pz54e4l6qhipchdxnaic4ssnuzloizvrasoq"
                },
                "linkedBlock": "omRkYXRhp2hmaWxlTmFtZW1jcmVhdGUgYSBmaWxlaGZpbGVUeXBlAGljb250ZW50SWR4P2tqemw2a2N5bTd3OHlhaXdqZWJtNW14N2Z4NWg5NHBjaGlrM21rZzd1YWt3NGN5b3B5czMwcWMwbWlmOXY4cmljcmVhdGVkQXR4GDIwMjMtMTAtMDdUMDg6MjI6NDYuMjA1Wmlmc1ZlcnNpb25kMC4xMWl1cGRhdGVkQXR4GDIwMjMtMTAtMDdUMDg6MjI6NDYuMjA1Wmtjb250ZW50VHlwZXiHZXlKeVpYTnZkWEpqWlNJNklrTkZVa0ZOU1VNaUxDSnlaWE52ZFhKalpVbGtJam9pYTJwNmJEWm9kbVp5WW5jMlkyRjBaV3N6Tm1nemNHVndNRGxyT1dkNWJXWnViR0U1YXpadmFteG5jbTEzYW05bmRtcHhaemh4TTNwd2VXSnNNWGwxSW4wZmhlYWRlcqRjc2VwZW1vZGVsZW1vZGVsWCjOAQIBhQESIH8JG4Y2KIV/LJ/ZtDn5+K80Ln63tgcVD+fDPvKyFFHIZnVuaXF1ZUzN9Y8RF8CouqUXYeZrY29udHJvbGxlcnOBeDtkaWQ6cGtoOmVpcDE1NToxOjB4NTkxNWUyOTM4MjNGQ2E4NDBjOTNFRDJFMUU1QjRkZjMyZDY5OTk5OQ",
                "cacaoBlock": "o2FooWF0Z2VpcDQzNjFhcKljYXVkeDhkaWQ6a2V5Ono2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVN2NleHB4GDIwMjMtMTAtMTRUMDc6Mjk6MjMuMTAyWmNpYXR4GDIwMjMtMTAtMDdUMDc6Mjk6MjMuMTAyWmNpc3N4O2RpZDpwa2g6ZWlwMTU1OjE6MHg1OTE1ZTI5MzgyM0ZDYTg0MGM5M0VEMkUxRTVCNGRmMzJkNjk5OTk5ZW5vbmNlbkRkbjdsU2MzdlFUd3F2ZmRvbWFpbnggY2VrcGZua2xjaWZpb21nZW9nYm1rbm5tY2dia2RwaW1ndmVyc2lvbmExaXJlc291cmNlc4p4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4c29nY2M0MzhmZ2dzdW55YnVxNnE5ZWN4b2FvemN4ZThxbGprOHd1M3VxdTM5NHV4N3hRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1eFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN3hsdGh6eDlkaXk2azNyM3MweGFmOGg3NG5neGhuY2dqd3llcGw1OHBrYTE1eDl5aGN4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4NjFjenZkc2xlZDN5bHNhOTk3N2k3cmxvd3ljOWw3anBnNmUxaGp3aDlmZWZsNmJzdXhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2I0bXNkODhpOG1sanp5cDNhencwOXgyNnYza2pvamVpdGJleDE4MWVmaTk0ZzU4ZWxmeFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN2d1ODhnNjZ6MjhuODFsY3BiZzZodTJ0OHB1MnB1aTBzZm5wdnNyaHFuM2t4aDl4YWl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhd3JsN2Y3NjdiNmN6NDhkbjBlZnI5d2Z0eDl0OWplbHc5dGIxb3R4ejc1MmpoODZrbnhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Yzg2Z3Q5ajQxNXl3Mng4c3Rta290Y3J6cGV1dHJia3A0Mmk0ejkwZ3A1aWJwdHo0c3NveFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjNnZiNjR3aTg4dWI0N2dibWNoODJ3Y3BibWU1MWh5bTRzOXFicDJ1a2FjMHl0aHpiajl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndWlzdGF0ZW1lbnR4MUdpdmUgdGhpcyBhcHBsaWNhdGlvbiBhY2Nlc3MgdG8gc29tZSBvZiB5b3VyIGRhdGFhc6Jhc3iEMHhmZDI0ZmVkNTA0MmFlMjdjYmY1NmUxN2FmNmJmZjdhNDQwZTZkMTY1NGZiNzhmZWQ4ZDNiYjdiN2RjOTRhMmFjMmY1MmU3M2EwMDdlZDhlMDExNzA2MGYyNzZjNTk2MTNhOGQ2OWI4NjgyNTJlYjZiMWE0MWE3ZGFkZWFlMzY3MzFiYXRmZWlwMTkx"
            },
            "opts": {
                "anchor": true,
                "publish": true,
                "sync": 3,
                "syncTimeoutSeconds": 0
            }
        }))?;

        println!("genesis: {:?}", genesis.stream_id()?.to_string());

        let result = client.save_genesis_commit(genesis).await;
        assert!(result.is_ok());

        let data: Data = serde_json::from_value(serde_json::json!({
            "streamId": "kjzl6kcym7w8y7aq5fcqraw3vk69f2syk6kpcmcs6xojujxf9batubj5ibki495",
            "commit": {
                "jws": {
                    "payload": "AXESIMnfzbG-k1039sJMGOiSotQoXSLkSd7sYRIx6socc21I",
                    "signatures": [
                        {
                            "protected": "eyJhbGciOiJFZERTQSIsImNhcCI6ImlwZnM6Ly9iYWZ5cmVpY3EzczJydmlzbGsycnRxajdqZTd4amlpYmNqN2ZjNmd4bHNtNGhmeGFzN3BnNmV4YzZ5bSIsImtpZCI6ImRpZDprZXk6ejZNa3REVkRVaEVhdUxiRUVaTVNBdFIxNzdkRHljZG96Y3hSZndQcVQyalFWSlU3I3o2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVNyJ9",
                            "signature": "wOjjvtKoyPl93aBmcCITwEiFqBTdGaYm9tkx0xyZPCngrzPeX5TYYWdXV1VLOvcc5aNnYU1fyqc3dRaoLV9SBA"
                        }
                    ],
                    "link": "bafyreigj37g3dputlu37nqsmddujfiwufbosfzcj33wgcerr5lfby43nja"
                },
                "linkedBlock": "o2JpZNgqWCYAAYUBEiBg5f963SDvWCdfJhnxwE7m0CIl1BNANfX9pvpPAPfmCWRkYXRhhKNib3BncmVwbGFjZWRwYXRoai91cGRhdGVkQXRldmFsdWV4GDIwMjMtMTAtMDdUMDg6MjM6MzIuMjI4WqNib3BncmVwbGFjZWRwYXRoaS9maWxlVHlwZWV2YWx1ZQKjYm9wZ3JlcGxhY2VkcGF0aGkvZmlsZU5hbWVldmFsdWV4K0xkNW83VDlJdUtKMW45UFpHTGdvcHoyOW1FTUI4Y3pQdTdWVUFrQ2xMTkGjYm9wY2FkZGRwYXRobi9hY2Nlc3NDb250cm9sZXZhbHVleQ2vZXlKbGJtTnllWEIwYVc5dVVISnZkbWxrWlhJaU9uc2ljSEp2ZEc5amIyd2lPaUpNYVhRaUxDSmxibU55ZVhCMFpXUlRlVzF0WlhSeWFXTkxaWGtpT2lKaE5EbGtNVGM1WXpSbU1UaGpNRFl6T1RKaVkySmlORFk0T0dWaFltVmtNV1ptTW1NME9EVmtNbUU0T0RsalpqUm1NakU1TkRBMU1XVXhZVGhsWVRWaE4yVXpNMkUxWW1OaE9EQXlaamt4TnpSak9HSmxNVGRsWWpjek1tTmpNVFV5TkdKbU1qUXhZak5rWVRrd09EbGpZVEkwTURZek16Y3paVEkyTm1KaU1HTmpOemt3TURNME1EaGxNelJtWm1WaVlqTXhNMlU0TXpFNU4ySTRPVFkzWkRVMU1UTTVaak5oTWpneE5XTXlOalZqT1RjeVl6YzNOakpsWkdVd1lUazBPRGhoTUdNM05HTm1OamMxT0RObVpqTTVNREl5WmpVeFlXSmxNRE5oTkRZMVpqUXpNemt4Tm1JME1qazVNR00zT1dRNE5XTmpZbU00TkRFd05XSmpNREF3TURBd01EQXdNREF3TURBeU1EYzRZV0ZsTW1Zd09EazBZamhsWVRZMlpqSXdPV0UxTVRkaVlURmhaak15T0dZeE5URTFObVU1TUdaaVpHTXlaRGxsTXpoaVptVTNNVGt3Wm1Wall6UTVaamM1TW1Ka1kyTTBObU15TldRMllUWmhOV1V3Wm1RM1lqYzVObVk1T0NJc0ltUmxZM0o1Y0hScGIyNURiMjVrYVhScGIyNXpJanBiZXlKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFFtRnphV01pTENKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJaUxDSnpkR0Z1WkdGeVpFTnZiblJ5WVdOMFZIbHdaU0k2SWxOSlYwVWlMQ0pqYUdGcGJpSTZJbVYwYUdWeVpYVnRJaXdpYldWMGFHOWtJam9pSWl3aWNHRnlZVzFsZEdWeWN5STZXeUk2Y21WemIzVnlZMlZ6SWwwc0luSmxkSFZ5YmxaaGJIVmxWR1Z6ZENJNmV5SmpiMjF3WVhKaGRHOXlJam9pWTI5dWRHRnBibk1pTENKMllXeDFaU0k2SW1ObGNtRnRhV002THk4cVAyMXZaR1ZzUFd0cWVtdzJhSFptY21KM05tTmhaM1EyT1RScGFXMHlkM1ZsWTNVM1pYVnRaV1J6TjNGa01IQTJkWHB0T0dSdWNYTnhOamxzYkRkcllXTnRNRFZuZFNKOWZTeDdJbTl3WlhKaGRHOXlJam9pWVc1a0luMHNleUpqYjI1a2FYUnBiMjVVZVhCbElqb2laWFp0UW1GemFXTWlMQ0pqYjI1MGNtRmpkRUZrWkhKbGMzTWlPaUlpTENKemRHRnVaR0Z5WkVOdmJuUnlZV04wVkhsd1pTSTZJbE5KVjBVaUxDSmphR0ZwYmlJNkltVjBhR1Z5WlhWdElpd2liV1YwYUc5a0lqb2lJaXdpY0dGeVlXMWxkR1Z5Y3lJNld5STZjbVZ6YjNWeVkyVnpJbDBzSW5KbGRIVnlibFpoYkhWbFZHVnpkQ0k2ZXlKamIyMXdZWEpoZEc5eUlqb2lZMjl1ZEdGcGJuTWlMQ0oyWVd4MVpTSTZJbU5sY21GdGFXTTZMeThxUDIxdlpHVnNQV3RxZW13MmFIWm1jbUozTm1NM1ozVTRPR2MyTm5veU9HNDRNV3hqY0dKbk5taDFNblE0Y0hVeWNIVnBNSE5tYm5CMmMzSm9jVzR6YTNob09YaGhhU0o5ZlN4N0ltOXdaWEpoZEc5eUlqb2lZVzVrSW4wc2V5SmpiMjVrYVhScGIyNVVlWEJsSWpvaVpYWnRRbUZ6YVdNaUxDSmpiMjUwY21GamRFRmtaSEpsYzNNaU9pSWlMQ0p6ZEdGdVpHRnlaRU52Ym5SeVlXTjBWSGx3WlNJNklsTkpWMFVpTENKamFHRnBiaUk2SW1WMGFHVnlaWFZ0SWl3aWJXVjBhRzlrSWpvaUlpd2ljR0Z5WVcxbGRHVnljeUk2V3lJNmNtVnpiM1Z5WTJWeklsMHNJbkpsZEhWeWJsWmhiSFZsVkdWemRDSTZleUpqYjIxd1lYSmhkRzl5SWpvaVkyOXVkR0ZwYm5NaUxDSjJZV3gxWlNJNkltTmxjbUZ0YVdNNkx5OHFQMjF2WkdWc1BXdHFlbXcyYUhabWNtSjNObU00Tm1kME9XbzBNVFY1ZHpKNE9ITjBiV3R2ZEdOeWVuQmxkWFJ5WW10d05ESnBOSG81TUdkd05XbGljSFI2TkhOemJ5SjlmU3g3SW05d1pYSmhkRzl5SWpvaVlXNWtJbjBzZXlKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFFtRnphV01pTENKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJaUxDSnpkR0Z1WkdGeVpFTnZiblJ5WVdOMFZIbHdaU0k2SWxOSlYwVWlMQ0pqYUdGcGJpSTZJbVYwYUdWeVpYVnRJaXdpYldWMGFHOWtJam9pSWl3aWNHRnlZVzFsZEdWeWN5STZXeUk2Y21WemIzVnlZMlZ6SWwwc0luSmxkSFZ5YmxaaGJIVmxWR1Z6ZENJNmV5SmpiMjF3WVhKaGRHOXlJam9pWTI5dWRHRnBibk1pTENKMllXeDFaU0k2SW1ObGNtRnRhV002THk4cVAyMXZaR1ZzUFd0cWVtdzJhSFptY21KM05tTmhkR1ZyTXpab00zQmxjREE1YXpsbmVXMW1ibXhoT1dzMmIycHNaM0p0ZDJwdlozWnFjV2M0Y1RONmNIbGliREY1ZFNKOWZTeDdJbTl3WlhKaGRHOXlJam9pWVc1a0luMHNXM3NpWTI5dVpHbDBhVzl1Vkhsd1pTSTZJbVYyYlVKaGMybGpJaXdpWTI5dWRISmhZM1JCWkdSeVpYTnpJam9pSWl3aWMzUmhibVJoY21SRGIyNTBjbUZqZEZSNWNHVWlPaUlpTENKamFHRnBiaUk2SW1WMGFHVnlaWFZ0SWl3aWJXVjBhRzlrSWpvaUlpd2ljR0Z5WVcxbGRHVnljeUk2V3lJNmRYTmxja0ZrWkhKbGMzTWlYU3dpY21WMGRYSnVWbUZzZFdWVVpYTjBJanA3SW1OdmJYQmhjbUYwYjNJaU9pSTlJaXdpZG1Gc2RXVWlPaUl3ZURVNU1UVmxNamt6T0RJelJrTmhPRFF3WXprelJVUXlSVEZGTlVJMFpHWXpNbVEyT1RrNU9Ua2lmWDBzZXlKdmNHVnlZWFJ2Y2lJNkltOXlJbjBzZXlKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJd2VFVkdPREUzTXpObU9USkRObU14TkVOaFl6azJNRGt4TnpoalJqaGhZemcwWWtaalpUSXlNallpTENKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFEyOXVkSEpoWTNRaUxDSm1kVzVqZEdsdmJrNWhiV1VpT2lKcGMwTnZiR3hsWTNSbFpDSXNJbVoxYm1OMGFXOXVVR0Z5WVcxeklqcGJJanAxYzJWeVFXUmtjbVZ6Y3lKZExDSm1kVzVqZEdsdmJrRmlhU0k2ZXlKcGJuQjFkSE1pT2x0N0ltbHVkR1Z5Ym1Gc1ZIbHdaU0k2SW1Ga1pISmxjM01pTENKdVlXMWxJam9pZFhObGNpSXNJblI1Y0dVaU9pSmhaR1J5WlhOekluMWRMQ0p1WVcxbElqb2lhWE5EYjJ4c1pXTjBaV1FpTENKdmRYUndkWFJ6SWpwYmV5SnBiblJsY201aGJGUjVjR1VpT2lKaWIyOXNJaXdpYm1GdFpTSTZJaUlzSW5SNWNHVWlPaUppYjI5c0luMWRMQ0p6ZEdGMFpVMTFkR0ZpYVd4cGRIa2lPaUoyYVdWM0lpd2lkSGx3WlNJNkltWjFibU4wYVc5dUluMHNJbU5vWVdsdUlqb2liWFZ0WW1GcElpd2ljbVYwZFhKdVZtRnNkV1ZVWlhOMElqcDdJbXRsZVNJNklpSXNJbU52YlhCaGNtRjBiM0lpT2lJOUlpd2lkbUZzZFdVaU9pSjBjblZsSW4xOVhWMHNJbVJsWTNKNWNIUnBiMjVEYjI1a2FYUnBiMjV6Vkhsd1pTSTZJbFZ1YVdacFpXUkJZMk5sYzNORGIyNTBjbTlzUTI5dVpHbDBhVzl1SW4wc0ltMXZibVYwYVhwaGRHbHZibEJ5YjNacFpHVnlJanA3SW5CeWIzUnZZMjlzSWpvaVRHVnVjeUlzSW1KaGMyVkRiMjUwY21GamRDSTZJakI0TnpVNE1qRTNOMFk1UlRVek5tRkNNR0kyWXpjeU1XVXhNV1l6T0RORE16STJSakpCWkRGRU5TSXNJblZ1YVc5dVEyOXVkSEpoWTNRaU9pSXdlRGMxT0RJeE56ZEdPVVUxTXpaaFFqQmlObU0zTWpGbE1URm1Nemd6UXpNeU5rWXlRV1F4UkRVaUxDSmphR0ZwYmtsa0lqbzRNREF3TVN3aVpHRjBZWFJ2YTJWdVNXUWlPaUl3ZUVWR09ERTNNek5tT1RKRE5tTXhORU5oWXprMk1Ea3hOemhqUmpoaFl6ZzBZa1pqWlRJeU1qWWlmWDBkcHJldtgqWCYAAYUBEiBg5f963SDvWCdfJhnxwE7m0CIl1BNANfX9pvpPAPfmCQ",
                "cacaoBlock": "o2FooWF0Z2VpcDQzNjFhcKljYXVkeDhkaWQ6a2V5Ono2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVN2NleHB4GDIwMjMtMTAtMTRUMDc6Mjk6MjMuMTAyWmNpYXR4GDIwMjMtMTAtMDdUMDc6Mjk6MjMuMTAyWmNpc3N4O2RpZDpwa2g6ZWlwMTU1OjE6MHg1OTE1ZTI5MzgyM0ZDYTg0MGM5M0VEMkUxRTVCNGRmMzJkNjk5OTk5ZW5vbmNlbkRkbjdsU2MzdlFUd3F2ZmRvbWFpbnggY2VrcGZua2xjaWZpb21nZW9nYm1rbm5tY2dia2RwaW1ndmVyc2lvbmExaXJlc291cmNlc4p4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4c29nY2M0MzhmZ2dzdW55YnVxNnE5ZWN4b2FvemN4ZThxbGprOHd1M3VxdTM5NHV4N3hRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1eFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN3hsdGh6eDlkaXk2azNyM3MweGFmOGg3NG5neGhuY2dqd3llcGw1OHBrYTE1eDl5aGN4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4NjFjenZkc2xlZDN5bHNhOTk3N2k3cmxvd3ljOWw3anBnNmUxaGp3aDlmZWZsNmJzdXhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2I0bXNkODhpOG1sanp5cDNhencwOXgyNnYza2pvamVpdGJleDE4MWVmaTk0ZzU4ZWxmeFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN2d1ODhnNjZ6MjhuODFsY3BiZzZodTJ0OHB1MnB1aTBzZm5wdnNyaHFuM2t4aDl4YWl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhd3JsN2Y3NjdiNmN6NDhkbjBlZnI5d2Z0eDl0OWplbHc5dGIxb3R4ejc1MmpoODZrbnhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Yzg2Z3Q5ajQxNXl3Mng4c3Rta290Y3J6cGV1dHJia3A0Mmk0ejkwZ3A1aWJwdHo0c3NveFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjNnZiNjR3aTg4dWI0N2dibWNoODJ3Y3BibWU1MWh5bTRzOXFicDJ1a2FjMHl0aHpiajl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndWlzdGF0ZW1lbnR4MUdpdmUgdGhpcyBhcHBsaWNhdGlvbiBhY2Nlc3MgdG8gc29tZSBvZiB5b3VyIGRhdGFhc6Jhc3iEMHhmZDI0ZmVkNTA0MmFlMjdjYmY1NmUxN2FmNmJmZjdhNDQwZTZkMTY1NGZiNzhmZWQ4ZDNiYjdiN2RjOTRhMmFjMmY1MmU3M2EwMDdlZDhlMDExNzA2MGYyNzZjNTk2MTNhOGQ2OWI4NjgyNTJlYjZiMWE0MWE3ZGFkZWFlMzY3MzFiYXRmZWlwMTkx"
            },
            "opts": {
                "anchor": true,
                "publish": true,
                "sync": 3
            }
        }))?;

        let result = client.save_data_commit(data).await;
        assert!(result.is_ok());

        let stream_id =
            "kjzl6kcym7w8y7aq5fcqraw3vk69f2syk6kpcmcs6xojujxf9batubj5ibki495".parse()?;
        let stream = client.load_streams2(&stream_id).await;
        assert!(stream.is_ok());

        let state: anyhow::Result<StreamState> = stream.unwrap().try_into();
        assert!(state.is_ok());

        println!("state: {:?}", serde_json::to_string(&state.unwrap())?);

        Ok(())
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
