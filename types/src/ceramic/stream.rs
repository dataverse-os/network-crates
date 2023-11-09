use std::str::FromStr;

use ceramic_core::Cid;
use ceramic_core::StreamId;
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::commit_id::CommitId;
use super::stream_id::StreamIdType;

/// Log entry for stream
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateLog {
    /// CID for commit
    pub cid: String,
    /// Type of commit
    pub r#type: u64,

    pub timestamp: Option<i64>,

    pub expiration_time: Option<i64>,
}

#[repr(u64)]
#[derive(Copy, Clone, Debug, Eq, IntEnum, PartialEq)]
pub enum LogType {
    Genesis = 0,
    Signed = 1,
    Anchor = 2,
}

#[repr(u64)]
#[derive(Copy, Clone, Debug, Eq, IntEnum, PartialEq)]
pub enum SignatureStatus {
    GENESIS = 0,
    PARTIAL = 1,
    SIGNED = 2,
}

/// Current state of stream
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamState {
    /// Type of stream
    pub r#type: u64,
    /// Content of stream
    pub content: Value,
    /// Log of stream
    pub log: Vec<StateLog>,
    /// Metadata for stream
    pub metadata: Value,
    // pub metadata: Metadata,
    /// Signature for stream
    pub signature: i32,
    /// Anchor status for stream
    pub anchor_status: AnchorStatus,
    /// Anchor proof for stream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor_proof: Option<AnchorProof>,
    /// Type of document
    pub doctype: String,
}

#[repr(u64)]
#[derive(Copy, Clone, Debug, Eq, IntEnum, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnchorStatus {
    NotRequested = 0,
    Pending = 1,
    Processing = 2,
    Anchored = 3,
    Failed = 4,
    Replaced = 5,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorProof {
    pub root: String,
    pub tx_hash: String,
    pub tx_type: Option<String>,
    pub chain_id: String,
}

impl StreamState {
    /// Get controllers for stream
    pub fn controllers(&self) -> Vec<String> {
        let mut controllers = vec![];
        if let Some(controllers_json) = self.metadata.get("controllers") {
            if let Some(controllers_vec) = controllers_json.as_array() {
                for controller in controllers_vec {
                    if let Some(controller_str) = controller.as_str() {
                        controllers.push(controller_str.to_string());
                    }
                }
            }
        }
        controllers
    }

    /// Get model id for stream
    pub fn model(&self) -> anyhow::Result<StreamId> {
        let model = self
            .metadata
            .get("model")
            .expect("model not found in metadata");
        let model = model.as_str().expect("model is not string");
        let model = StreamId::from_str(model)?;
        Ok(model)
    }

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        let cid = &self.log.first().expect("log is empty").cid;
        Ok(StreamId {
            r#type: StreamIdType::try_from(self.r#type)?.into(),
            cid: Cid::from_str(cid.as_ref())?,
        })
    }

    pub fn commit_ids(&self) -> anyhow::Result<Vec<CommitId>> {
        let mut commit_ids = vec![];
        let stream_id = self.stream_id()?;
        for log in self.log.iter() {
            let commit_id = CommitId {
                stream_id: stream_id.clone(),
                tip: Cid::from_str(log.cid.as_ref())?,
            };
            commit_ids.push(commit_id);
        }
        Ok(commit_ids)
    }
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            r#type: 0,
            content: Default::default(),
            metadata: Default::default(),
            signature: 2,
            anchor_status: AnchorStatus::Pending,
            log: vec![],
            doctype: "MID".to_string(),
            anchor_proof: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::ceramic::stream::StreamState;

    use super::*;

    #[test]
    fn test_serialize_anchor_status() {
        let status = AnchorStatus::Anchored;
        let status = serde_json::to_value(&status);
        assert!(status.is_ok());
        let status = status.unwrap();
        assert_eq!(status, json!("ANCHORED"));
    }

    #[test]
    fn test_deserialize_anchor_status() {
        let status = json!("ANCHORED");
        let status = serde_json::from_value::<AnchorStatus>(status);
        assert!(status.is_ok());
        let status = status.unwrap();
        assert_eq!(status, AnchorStatus::Anchored);
    }

    #[test]
    fn test_decode_from_json() {
        let data = json!({
          "type": 3,
          "content": {
            "comment": "eyJtaXJyb3JOYW1lIjoicG9zdCIsIm5vdGUiOiIiLCJ0YWdzIjpbXX0",
            "fileType": 0,
            "contentId": "kjzl6kcym7w8y5fhg4cl0xi8npfke3jmaaeeic9dwx64bgunnqa6amortdgdsym",
            "createdAt": "2023-04-04T07:25:14.877Z",
            "updatedAt": "2023-04-04T07:25:14.877Z",
            "appVersion": "0.2.0",
            "contentType": "kjzl6hvfrbw6cb1jfm9wiuqelthhvv3hzpb2urkbcwdum1g0ao2qygdj0qdqn5g"
          },
          "metadata": {
            "controllers": [
              "did:pkh:eip155:137:0x312eA852726E3A9f633A0377c0ea882086d66666"
            ],
            "model": "kjzl6hvfrbw6c763ubdhowzao0m4yp84cxzbfnlh4hdi5alqo4yrebmc0qpjdi5"
          },
          "signature": 2,
          "anchorStatus": "ANCHORED",
          "log": [
            {
              "cid": "bagcqcerayswtqarydm2rgeh37yir45ccvfkj3qhwhfmu4vdjjrtny5l4rpia",
              "type": 0,
              "expirationTime": 1681197855,
              "timestamp": 1680629255
            },
            {
              "cid": "bafyreid43i4yornrup5nuiiu5bavu3k5se4z7wrokwd2oznvanp27eo7xe",
              "type": 2,
              "timestamp": 1680629255
            }
          ],
          "anchorProof": {
            "root": "bafyreihtmj5y6lbm23uulkwddp2hdiw4frhe6ofiunoqqjkcxasvuxlbrq",
            "txHash": "bagjqcgzaxm7xafnnfsvocyf7sya7m5qm64jztmcvwykwn3q6uvyw52mj6iua",
            "txType": "f(bytes32)",
            "chainId": "eip155:1"
          },
          "doctype": "MID"
        });
        let data = serde_json::from_value::<StreamState>(data);

        assert!(data.is_ok());
        let data = data.unwrap();
        assert_eq!(
            data.controllers(),
            vec!["did:pkh:eip155:137:0x312eA852726E3A9f633A0377c0ea882086d66666"]
        );
        assert_eq!(
            data.model().unwrap().to_string(),
            "kjzl6hvfrbw6c763ubdhowzao0m4yp84cxzbfnlh4hdi5alqo4yrebmc0qpjdi5"
        );

        let stream_id = data.stream_id().unwrap().to_string();
        assert_eq!(
            stream_id,
            "kjzl6kcym7w8y9s94kcardbh5u0ao76bci07xnnxjw1ew3i4eackykj76uagqfk"
        );
        assert_eq!(
            data.commit_ids().unwrap(),
            vec![
                CommitId::from_str(
                    &stream_id,
                    "bagcqcerayswtqarydm2rgeh37yir45ccvfkj3qhwhfmu4vdjjrtny5l4rpia"
                )
                .unwrap(),
                CommitId::from_str(
                    &stream_id,
                    "bafyreid43i4yornrup5nuiiu5bavu3k5se4z7wrokwd2oznvanp27eo7xe"
                )
                .unwrap()
            ],
        );
    }
}
