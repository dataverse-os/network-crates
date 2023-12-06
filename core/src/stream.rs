use ceramic_core::{Cid, StreamId};
use chrono::{DateTime, Utc};
use dataverse_ceramic::{
    event::{self, Event},
    StreamState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub r#type: u64,
    pub dapp_id: uuid::Uuid,
    // pub network: String,
    pub expiration_time: Option<DateTime<Utc>>,
    pub genesis: Cid,
    pub tip: Cid,
    pub model: Option<StreamId>,
    pub published: usize,
}

impl Stream {
    pub fn new(
        dapp_id: &uuid::Uuid,
        r#type: u64,
        genesis: &event::Event,
        model: Option<StreamId>,
    ) -> anyhow::Result<Self> {
        if let event::EventValue::Signed(signed) = &genesis.value {
            let expiration_time = match signed.cacao()? {
                Some(cacao) => cacao.p.expiration_time()?,
                None => None,
            };
            return Ok(Stream {
                r#type,
                dapp_id: dapp_id.clone(),
                expiration_time,
                published: 0,
                tip: genesis.cid,
                genesis: genesis.cid,
                model: model,
            });
        }
        anyhow::bail!("invalid genesis commit");
    }

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        Ok(StreamId {
            r#type: self.r#type.try_into()?,
            cid: self.genesis,
        })
    }

    pub async fn state(&self, commits: Vec<Event>) -> anyhow::Result<StreamState> {
        StreamState::make(self.r#type, commits).await
    }
}

#[async_trait::async_trait]
pub trait StreamStore {
    async fn save_stream(&self, stream: &Stream) -> anyhow::Result<()>;
    async fn load_stream(&self, stream_id: &StreamId) -> anyhow::Result<Option<Stream>>;
}
