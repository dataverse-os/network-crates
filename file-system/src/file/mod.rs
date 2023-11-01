mod client;
mod loader;

pub use client::*;
pub use loader::*;

use ceramic_core::{Cid, StreamId, StreamIdType};
use serde::{Deserialize, Serialize};
use serde_json::value::Value;

use crate::index_file::IndexFile;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_model_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<IndexFile>,
    pub content_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    pub controller: String,
    pub verified_status: i64,
}

impl Default for StreamFile {
    fn default() -> Self {
        Self {
            file_id: Some(StreamId {
                r#type: StreamIdType::Tile,
                cid: Cid::default(),
            }),
            file_model_id: Some(StreamId {
                r#type: StreamIdType::Tile,
                cid: Cid::default(),
            }),
            file: Default::default(),
            content_id: Default::default(),
            model_id: Default::default(),
            content: Default::default(),
            verified_status: Default::default(),
            controller: Default::default(),
        }
    }
}
