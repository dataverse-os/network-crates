use anyhow::{Context, Result};
use ceramic_core::{Base64UrlString, Cid, StreamId};
use ceramic_event::{DidDocument, JwkSigner};
use ceramic_http_client::{remote::CeramicRemoteHttpClient, FilterQuery};
use int_enum::IntEnum;
use json_patch::{patch, Patch};
use ssi::jwk::Algorithm;

use crate::{
    commit::{Data, Genesis},
    did::generate_did_str,
    event::{Event, EventsLoader, EventsUploader},
    network::{Chain, Network},
    stream::StreamState,
    Ceramic, LogType, StreamLoader, StreamOperator, StreamsLoader,
};

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn init(ceramic: &str) -> anyhow::Result<CeramicHTTPClient> {
        let ceramic_url = url::Url::parse(ceramic)?;
        Ok(CeramicRemoteHttpClient::new(NullSigner::new(), ceramic_url))
    }

    pub async fn query_model(
        &self,
        ceramic: &Ceramic,
        account: Option<String>,
        model_id: &StreamId,
        query: Option<FilterQuery>,
    ) -> anyhow::Result<Vec<StreamState>> {
        let http_client = Self::init(&ceramic.endpoint)?;
        let edges = http_client.query_all(account, model_id, query).await?;
        let mut streams = Vec::new();
        for edge in edges {
            if let Some(node) = edge.node {
                streams.push(node.try_into()?);
            }
        }
        Ok(streams)
    }

    pub async fn chains(ceramic: &str) -> anyhow::Result<Vec<Chain>> {
        let http_client = Self::init(ceramic)?;
        let chains = http_client.chains().await?.supported_chains;
        let chains = chains
            .iter()
            .map(|ele| ele.parse())
            .collect::<Result<Vec<Chain>, _>>()?;
        Ok(chains)
    }

    pub async fn network(ceramic: &str) -> anyhow::Result<Network> {
        let http_client = Self::init(ceramic)?;
        let chains = http_client.chains().await?.supported_chains;
        let chain = chains
            .first()
            .context("ceramic not in networks")?
            .parse::<Chain>()?;
        Ok(chain.network())
    }
}

#[async_trait::async_trait]
impl EventsLoader for Client {
    async fn load_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        _tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>> {
        let http_client = Self::init(&ceramic.endpoint)?;
        let commits = http_client.commits(stream_id).await?.commits;
        let mut events = vec![];
        for commit in commits {
            events.push(commit.try_into()?)
        }
        Ok(events)
    }
}

#[async_trait::async_trait]
impl EventsUploader for Client {
    async fn upload_event(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()> {
        let http_client = Self::init(&ceramic.endpoint)?;
        let client = reqwest::Client::new();
        match commit.log_type() {
            LogType::Genesis => {
                let url = http_client.url_for_path("/api/v0/streams")?;
                let genesis = Genesis {
                    r#type: stream_id.r#type.int_value(),
                    genesis: commit.clone().try_into()?,
                    opts: serde_json::Value::Null,
                };
                match client.post(url.as_str()).json(&genesis).send().await {
                    Ok(res) => log::debug!("publish genesis {:?}", res),
                    Err(err) => log::error!("publish genesis {}", err),
                };
            }
            LogType::Signed => {
                let url = http_client.url_for_path("/api/v0/commits")?;
                let signed = Data {
                    stream_id: stream_id.clone(),
                    commit: commit.clone().try_into()?,
                    opts: serde_json::Value::Null,
                };
                match client.post(url.as_str()).json(&signed).send().await {
                    Ok(res) => log::debug!("publish signed {:?}", res),
                    Err(err) => log::error!("publish signed {}", err),
                };
            }
            _ => anyhow::bail!("invalid log type"),
        };
        Ok(())
    }
}

#[async_trait::async_trait]
impl StreamOperator for Client {}

#[async_trait::async_trait]
impl StreamLoader for Client {
    async fn load_stream_state(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        _tip: Option<Cid>,
    ) -> anyhow::Result<StreamState> {
        let ceramic = Self::init(&ceramic.endpoint)?;
        let stream = ceramic.get(stream_id).await?;
        let state = stream.state.context("Failed to load stream")?.try_into()?;
        Ok(state)
    }
}

#[async_trait::async_trait]
impl StreamsLoader for Client {
    async fn load_stream_states(
        &self,
        ceramic: &Ceramic,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        self.query_model(ceramic, account, model_id, None).await
    }
}

pub async fn ceramic_client(ceramic: &str, pk: &str) -> Result<CeramicRemoteHttpClient<JwkSigner>> {
    let did = generate_did_str(pk)?;
    let did = DidDocument::new(&did);
    let signer = JwkSigner::new(did, pk).await?;

    let ceramic_url = url::Url::parse(ceramic)?;
    Ok(CeramicRemoteHttpClient::new(signer, ceramic_url))
}

type CeramicHTTPClient = CeramicRemoteHttpClient<NullSigner>;

pub struct NullSigner;

impl NullSigner {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ceramic_http_client::ceramic_event::Signer for NullSigner {
    fn algorithm(&self) -> Algorithm {
        Algorithm::EdDSA
    }

    fn id(&self) -> &DidDocument {
        todo!()
    }

    async fn sign(&self, _bytes: &[u8]) -> anyhow::Result<Base64UrlString> {
        anyhow::bail!("NullSigner cannot sign")
    }
}

pub trait StreamStateTrait {
    fn apply_patch(&mut self, patches: Patch) -> Result<()>;
}

impl StreamStateTrait for StreamState {
    fn apply_patch(&mut self, patches: Patch) -> Result<()> {
        patch(&mut self.content, &patches)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ceramic_core::StreamId;
    use int_enum::IntEnum;
    use serde_json::{from_value, json};
    use std::str::FromStr;

    #[test]
    fn test_apply_patch() {
        // Test applying a single patch
        let mut state = StreamState {
            content: json!({
                "key": "value1",
            }),
            ..Default::default()
        };
        let patch: Patch = from_value(json!([
            { "op": "replace", "path": "/key", "value": "value2" },
        ]))
        .unwrap();
        state.apply_patch(patch).unwrap();
        assert_eq!(state.content, json!({"key": "value2"}));
    }

    #[test]
    fn test_metadata() {
        let metadata = json!({
          "controllers": [
            "did:pkh:eip155:1:0x312eA852726E3A9f633A0377c0ea882086d66666"
          ],
          "model": "kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso"
        });

        let state = StreamState {
            metadata,
            ..Default::default()
        };

        assert_eq!(
            state.controllers(),
            vec!["did:pkh:eip155:1:0x312eA852726E3A9f633A0377c0ea882086d66666"]
        );

        assert_eq!(
            state.model().unwrap(),
            StreamId::from_str("kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso")
                .unwrap()
        );
    }

    #[tokio::test]
    async fn load_events() {
        let client = Client::new();
        let ceramic = "https://dataverseceramicdaemon.com";
        let http_client = Client::init(ceramic);
        assert!(http_client.is_ok());
        let http_client = http_client.unwrap();

        let ceramic = Ceramic::new(ceramic).await;
        assert!(ceramic.is_ok());

        let stream_id =
            StreamId::from_str("kjzl6kcym7w8y5pj1xs5iotnbplg7x4hgoohzusuvk8s7oih3h2fuplcvwvu2wx")
                .unwrap();
        let events = client
            .load_events(&ceramic.unwrap(), &stream_id, None)
            .await;
        assert!(events.is_ok());

        let stream = StreamState::new(stream_id.r#type.int_value(), events.unwrap());
        assert!(stream.is_ok());

        let stream_from_ceramic = http_client.get(&stream_id).await;
        assert!(stream_from_ceramic.is_ok());

        assert_eq!(
            stream.unwrap().content,
            stream_from_ceramic.unwrap().state.unwrap().content
        );
    }
}
