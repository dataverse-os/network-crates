use base64::Engine;
use libipld::cbor::DagCborCodec;
use libipld::codec::Codec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageResponse {
    pub from: String,
    pub data: String,
    pub seqno: String,
    #[serde(rename = "topicIDs")]
    pub topic_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, libipld::DagCbor, PartialEq, Eq)]
pub struct MessageQuery {
    #[ipld]
    pub tpy: i32,
    #[ipld]
    pub stream: String,
}

pub fn message_hash(tpy: i32, stream: String) -> anyhow::Result<String> {
    let obj = MessageQuery { tpy, stream };
    let res = DagCborCodec.encode(&obj)?;
    let mut hasher = Sha256::new();
    hasher.update(res);
    let mut id: Vec<u8> = hasher.finalize().to_vec();
    let mut digest = vec![0x12, id.len() as u8];
    digest.append(&mut id);
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest))
}
