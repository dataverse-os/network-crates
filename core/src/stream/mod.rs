mod loader;
mod publisher;

pub use loader::*;
pub use publisher::*;

use ceramic_core::{Cid, StreamId};
use chrono::{DateTime, Utc};
use dataverse_ceramic::event;
use serde::{Deserialize, Serialize};

pub trait StreamOperator: StreamLoader + StreamPublisher {}

#[async_trait::async_trait]
impl StreamOperator for () {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub r#type: u64,
    pub dapp_id: uuid::Uuid,
    // pub network: String,
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

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        Ok(StreamId {
            r#type: self.r#type.try_into()?,
            cid: self.genesis,
        })
    }
}
