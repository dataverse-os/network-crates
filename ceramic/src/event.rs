use anyhow::Result;
use ceramic_core::{Base64String, Base64UrlString, StreamId};
use chrono::{DateTime, Utc};
use dag_jose::DagJoseCodec;
use dataverse_types::ceramic::{LogType, StateLog, StreamState};
use json_patch::Patch;
use libipld::prelude::Codec;
use libipld::{cbor::DagCborCodec, cid::Cid, json::DagJsonCodec, Ipld};
use serde::{Deserialize, Serialize};

use crate::cacao::CACAO;

use super::jws::ToCid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub cid: Cid,
    pub value: EventValue,
}

impl Event {
    pub fn prev(&self) -> anyhow::Result<Option<Cid>> {
        match &self.value {
            EventValue::Signed(e) => Ok(e.payload()?.prev),
            EventValue::Anchor(e) => Ok(Some(e.prev)),
        }
    }

    pub fn verify_signature(
        &self,
        opts: Vec<VerifyOption>,
    ) -> anyhow::Result<Option<DateTime<Utc>>> {
        let mut expiration_time = None;
        if let EventValue::Signed(signed) = &self.value {
            if let Some(cacao) = signed.cacao()? {
                for ele in opts {
                    match ele {
                        VerifyOption::ResourceModelsContain(model) => {
                            let resource_models = cacao.p.resource_models()?;
                            if !resource_models.contains(&model) {
                                anyhow::bail!("invalid resource model");
                            }
                        }
                        VerifyOption::ExpirationTimeBefore(before) => {
                            expiration_time = cacao.p.expiration_time()?;
                            if let Some(exp) = expiration_time {
                                if exp < before {
                                    anyhow::bail!("jws commit expired");
                                }
                            }
                        }
                    }
                }
            };
        };
        Ok(expiration_time)
    }

    pub fn log_type(&self) -> LogType {
        match &self.value {
            EventValue::Signed(signed) => match signed.is_gensis() {
                true => LogType::Genesis,
                false => LogType::Signed,
            },
            EventValue::Anchor(_) => LogType::Anchor,
        }
    }

    pub fn apply_to(&self, state: &mut StreamState) -> anyhow::Result<()> {
        self.value.apply_to(state)?;
        let (ts, exp) = match &self.value {
            EventValue::Signed(signed) => {
                if let Some(cacao) = signed.cacao()? {
                    (None, cacao.p.expiration_time()?)
                } else {
                    (None, None)
                }
            }
            EventValue::Anchor(_) => (None, None),
        };
        let state_log = StateLog {
            cid: self.cid.to_string(),
            r#type: self.log_type() as u64,
            timestamp: ts,
            expiration_time: exp.map(|t| t.timestamp()),
        };
        state.log.push(state_log);
        Ok(())
    }

    pub fn decode(cid: Cid, data: Vec<u8>) -> anyhow::Result<Self> {
        let codec = cid.codec();
        let value = EventValue::decode(codec, data)?;
        Ok(Event { cid, value })
    }
}

impl TryFrom<ceramic_http_client::api::Commit> for Event {
    type Error = anyhow::Error;

    fn try_from(value: ceramic_http_client::api::Commit) -> std::result::Result<Self, Self::Error> {
        match value.value {
            ceramic_http_client::api::CommitValue::Anchor(anchor) => Ok(Event {
                cid: value.cid.as_ref().try_into()?,
                value: EventValue::Anchor(AnchorValue {
                    id: anchor.id.as_ref().try_into()?,
                    prev: anchor.prev.as_ref().try_into()?,
                    proof: anchor.proof.as_ref().try_into()?,
                    path: anchor.path,
                }),
            }),
            ceramic_http_client::api::CommitValue::Signed(signed) => Ok(Event {
                cid: value.cid.as_ref().try_into()?,
                value: EventValue::Signed(SignedValue {
                    jws: signed.jws,
                    linked_block: Some(signed.linked_block.to_vec()?),
                    cacao_block: None,
                }),
            }),
        }
    }
}

impl TryFrom<ceramic_core::Jws> for Event {
    type Error = anyhow::Error;

    fn try_from(jws: ceramic_core::Jws) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            cid: jws.cid()?,
            value: EventValue::Signed(SignedValue {
                jws,
                linked_block: None,
                cacao_block: None,
            }),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventValue {
    Signed(SignedValue),
    Anchor(AnchorValue),
}

impl EventValue {
    pub fn apply_to(&self, stream_state: &mut StreamState) -> anyhow::Result<()> {
        if let Self::Signed(e) = self {
            if let Ok(payload) = &e.payload() {
                match payload.id.is_none() {
                    // gensis commit
                    true => {
                        if let Some(data) = &payload.data {
                            stream_state.content = data.clone();
                        }
                        if let Some(header) = &payload.header {
                            stream_state.metadata = header.to_metadata();
                        }
                    }
                    // data commit
                    false => {
                        if let Some(data) = &payload.data {
                            let patch: json_patch::Patch = serde_json::from_value(data.clone())?;
                            json_patch::patch(&mut stream_state.content, &patch)?;
                        }
                    }
                }
            };
        };
        Ok(())
    }

    pub fn decode(codec: u64, data: Vec<u8>) -> Result<Self> {
        match codec {
            0x71 => Ok(EventValue::Anchor(
                libipld::serde::from_ipld::<AnchorValue>(DagCborCodec.decode(&data)?)?,
            )),
            0x85 => Ok(EventValue::Signed(data.decode()?)),
            _ => anyhow::bail!("unsupported codec {}", codec),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedValue {
    pub jws: ceramic_core::Jws,
    pub linked_block: Option<Vec<u8>>,
    pub cacao_block: Option<Vec<u8>>,
}

impl Clone for SignedValue {
    fn clone(&self) -> Self {
        Self {
            jws: ceramic_core::Jws {
                link: self.jws.link.clone(),
                payload: self.jws.payload.clone(),
                signatures: self
                    .jws
                    .signatures
                    .iter()
                    .map(|sig| ceramic_core::JwsSignature {
                        protected: sig.protected.clone(),
                        signature: sig.signature.clone(),
                    })
                    .collect::<Vec<_>>(),
            },
            linked_block: self.linked_block.clone(),
            cacao_block: self.cacao_block.clone(),
        }
    }
}

impl SignedValue {
    pub fn payload(&self) -> anyhow::Result<Payload> {
        if let Some(linked_block) = self.linked_block.clone() {
            let payload = Payload::try_from(linked_block)?;
            Ok(payload)
        } else {
            anyhow::bail!("linked_block is none")
        }
    }

    pub fn payload_link(&self) -> anyhow::Result<Cid> {
        return Ok(Cid::try_from(self.jws.payload.to_vec()?)?);
    }

    pub fn cacao(&self) -> anyhow::Result<Option<CACAO>> {
        if let Some(cacao_block) = self.cacao_block.clone() {
            let node: Ipld = DagCborCodec.decode(&cacao_block)?;
            let cacao: CACAO = libipld::serde::from_ipld(node)?;
            return Ok(Some(cacao));
        }
        Ok(None)
    }

    pub fn data(&self) -> anyhow::Result<serde_json::Value> {
        match &self.payload()?.data {
            Some(data) => Ok(data.clone()),
            None => anyhow::bail!("data is none"),
        }
    }

    pub fn patch(&self) -> anyhow::Result<Patch> {
        match &self.payload()?.data {
            Some(data) => Ok(serde_json::from_value(data.clone())?),
            None => anyhow::bail!("data is none"),
        }
    }

    pub fn is_gensis(&self) -> bool {
        match &self.payload() {
            Ok(payload) => payload.id.is_none(),
            _ => false,
        }
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnchorValue {
    pub id: Cid,
    pub prev: Cid,
    pub proof: Cid,
    pub path: String,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    pub model: StreamId,
    pub controllers: Vec<String>,
    pub unique: Vec<u8>,
}

impl Header {
    pub fn to_metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "model": self.model.to_string(),
            "controllers": self.controllers,
        })
    }
}

impl TryFrom<&Ipld> for Header {
    type Error = anyhow::Error;

    fn try_from(node: &Ipld) -> std::result::Result<Self, Self::Error> {
        let model: Vec<u8> = node.get("model")?.as_some().expect("model not found");

        let mut controllers = Vec::new();

        if let Ipld::List(list) = node.get("controllers")? {
            for ele in list {
                controllers.push(ele.as_some().expect("failed to parse controller as string"))
            }
        }

        Ok(Header {
            model: StreamId::try_from(model.as_slice())?,
            controllers,
            unique: node
                .get("unique")?
                .as_some()
                .expect("failed to parse unique"),
        })
    }
}

pub enum VerifyOption {
    ResourceModelsContain(StreamId),
    ExpirationTimeBefore(DateTime<Utc>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Payload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<Header>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev: Option<Cid>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Cid>,
}

impl IpldDecodeFrom<Payload> for Vec<u8> {
    fn decode(&self) -> Result<Payload> {
        let node: Ipld = DagCborCodec.decode(&self)?;
        Ok(TryFrom::try_from(&node)?)
    }
}

impl TryFrom<Vec<u8>> for Payload {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        IpldDecodeFrom::<Payload>::decode(&value)
    }
}

impl TryFrom<Base64String> for Payload {
    type Error = anyhow::Error;

    fn try_from(value: Base64String) -> std::result::Result<Self, Self::Error> {
        value.to_vec()?.try_into()
    }
}

impl TryFrom<&Ipld> for Payload {
    type Error = anyhow::Error;

    fn try_from(node: &Ipld) -> Result<Self, Self::Error> {
        let data = match node.get("data") {
            Ok(data) => Some(serde_json::from_slice(
                DagJsonCodec.encode(&data)?.as_slice(),
            )?),
            Err(_) => None,
        };

        let header = node
            .get("header")
            .ok()
            .and_then(|header| header.try_into().ok());
        let prev = node.get("prev").ok().and_then(IpldAs::as_some);
        let id = node.get("id").ok().and_then(IpldAs::as_some);

        Ok(Payload {
            data,
            header,
            prev,
            id,
        })
    }
}

impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.header == other.header
            && self.prev == other.prev
            && self.id == other.id
    }
}

trait DecodeFromIpld {
    fn decode_signed_event(data: Vec<u8>) -> Result<Event>;
}

pub trait IpldDecodeFrom<T> {
    fn decode(&self) -> Result<T>;
}

impl IpldDecodeFrom<SignedValue> for Vec<u8> {
    fn decode(&self) -> Result<SignedValue> {
        let node: Ipld = DagJoseCodec.decode(&self)?;
        Ok(SignedValue {
            jws: node.decode_jws()?,
            linked_block: None,
            cacao_block: None,
        })
    }
}

trait IpldAsJws {
    fn decode_jws(&self) -> Result<ceramic_core::Jws>;
}

impl IpldAsJws for Ipld {
    fn decode_jws(&self) -> Result<ceramic_core::Jws> {
        let payload: Vec<u8> = self
            .get("payload")?
            .as_some()
            .expect("failed to get payload");
        let signatures: Vec<ceramic_core::JwsSignature> = match self.get("signatures")? {
            Ipld::List(body) => Some(body.into_iter().map(|sig| {
                let protected: Vec<u8> = sig.get("protected").unwrap().as_some().unwrap();
                let signature: Vec<u8> = sig.get("signature").unwrap().as_some().unwrap();
                ceramic_core::JwsSignature {
                    protected: Some(Base64String::from(protected)),
                    signature: Base64UrlString::from(signature),
                }
            })),
            _ => None,
        }
        .unwrap()
        .collect();

        Ok(ceramic_core::Jws {
            link: None,
            payload: Base64UrlString::from(payload),
            signatures,
        })
    }
}

trait IpldAs<T> {
    fn as_some(&self) -> Option<T>;
}

impl IpldAs<Vec<u8>> for Ipld {
    fn as_some(&self) -> Option<Vec<u8>> {
        match self {
            Ipld::Bytes(body) => Some(body.clone()),
            _ => None,
        }
    }
}

impl IpldAs<Cid> for Ipld {
    fn as_some(&self) -> Option<Cid> {
        match self {
            Ipld::Link(link) => Some(link.clone()),
            _ => None,
        }
    }
}

impl IpldAs<String> for Ipld {
    fn as_some(&self) -> Option<String> {
        match self {
            Ipld::String(str) => Some(str.clone()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_decode_payload_base64() {
        let data = vec![
            163, 98, 105, 100, 216, 42, 88, 38, 0, 1, 133, 1, 18, 32, 33, 38, 237, 255, 127, 123,
            236, 38, 58, 201, 108, 181, 208, 161, 100, 94, 80, 252, 94, 33, 225, 14, 150, 95, 58,
            6, 199, 144, 171, 139, 89, 65, 100, 100, 97, 116, 97, 128, 100, 112, 114, 101, 118,
            216, 42, 88, 38, 0, 1, 133, 1, 18, 32, 23, 56, 209, 176, 253, 43, 123, 225, 127, 15,
            21, 160, 176, 202, 178, 72, 62, 6, 227, 22, 240, 144, 175, 153, 131, 171, 244, 79, 205,
            142, 180, 163,
        ];
        let result = Payload::try_from(data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_anchor_event() {
        // Test data
        let data = vec![
            164, 98, 105, 100, 216, 42, 88, 38, 0, 1, 133, 1, 18, 32, 254, 228, 97, 178, 152, 203,
            84, 252, 9, 177, 84, 148, 173, 107, 75, 252, 41, 230, 17, 181, 141, 244, 177, 224, 141,
            77, 198, 201, 12, 1, 141, 61, 100, 112, 97, 116, 104, 113, 48, 47, 48, 47, 48, 47, 49,
            47, 48, 47, 48, 47, 48, 47, 48, 47, 49, 100, 112, 114, 101, 118, 216, 42, 88, 38, 0, 1,
            133, 1, 18, 32, 44, 67, 74, 135, 93, 211, 163, 51, 90, 195, 121, 33, 70, 249, 27, 11,
            125, 248, 93, 7, 114, 194, 82, 233, 17, 100, 13, 7, 248, 44, 157, 233, 101, 112, 114,
            111, 111, 102, 216, 42, 88, 37, 0, 1, 113, 18, 32, 115, 27, 196, 150, 174, 107, 249,
            27, 43, 77, 252, 151, 46, 25, 88, 129, 238, 90, 58, 229, 7, 70, 5, 149, 238, 126, 123,
            225, 80, 136, 201, 125,
        ];

        let node: Ipld = DagCborCodec.decode(&data).unwrap();
        let result = libipld::serde::from_ipld::<AnchorValue>(node);
        assert!(result.is_ok());
        let result = result.unwrap();

        let expected = AnchorValue {
            id: Cid::from_str("bagcqcera73sgdmuyznkpycnrkskk222l7qu6menvrx2ldyenjxdmsdabru6q")
                .unwrap(),
            prev: Cid::from_str("bagcqcerafrbuvb252ortgwwdpequn6i3bn67qxiholbff2irmqgqp6bmtxuq")
                .unwrap(),
            proof: Cid::from_str("bafyreidtdpcjnltl7enswtp4s4xbsweb5zndvzihiyczl3t6ppqvbcgjpu")
                .unwrap(),
            path: "0/0/0/1/0/0/0/0/1".to_string(),
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn test_decode_payload_valid_data() {
        let data = vec![
            162, 100, 100, 97, 116, 97, 166, 103, 111, 112, 116, 105, 111, 110, 115, 121, 10, 38,
            101, 121, 74, 109, 98, 50, 120, 107, 90, 88, 74, 79, 89, 87, 49, 108, 73, 106, 111,
            105, 86, 87, 53, 48, 97, 88, 82, 115, 90, 87, 81, 105, 76, 67, 74, 109, 98, 50, 120,
            107, 90, 88, 74, 69, 90, 88, 78, 106, 99, 109, 108, 119, 100, 71, 108, 118, 98, 105,
            73, 54, 73, 105, 73, 115, 73, 109, 86, 117, 89, 51, 74, 53, 99, 72, 82, 108, 90, 70,
            78, 53, 98, 87, 49, 108, 100, 72, 74, 112, 89, 48, 116, 108, 101, 83, 73, 54, 73, 109,
            77, 51, 90, 84, 90, 109, 78, 68, 107, 52, 79, 68, 85, 121, 90, 87, 89, 121, 90, 87, 77,
            52, 77, 84, 81, 119, 89, 122, 90, 104, 89, 106, 99, 119, 90, 68, 100, 106, 89, 87, 85,
            52, 89, 109, 81, 119, 90, 106, 77, 53, 78, 71, 81, 52, 78, 122, 104, 106, 77, 68, 77,
            52, 79, 84, 78, 106, 90, 71, 82, 105, 77, 87, 85, 53, 89, 106, 108, 104, 77, 122, 65,
            49, 77, 84, 74, 104, 90, 71, 69, 121, 90, 68, 85, 121, 78, 122, 99, 122, 90, 84, 74,
            109, 78, 87, 77, 53, 89, 122, 66, 107, 77, 84, 85, 48, 77, 106, 78, 105, 77, 106, 78,
            104, 77, 109, 81, 119, 90, 87, 82, 107, 90, 71, 85, 53, 89, 84, 99, 122, 89, 106, 77,
            48, 89, 50, 74, 108, 77, 109, 77, 51, 77, 68, 82, 107, 77, 106, 73, 50, 77, 84, 89, 48,
            77, 122, 70, 104, 78, 68, 65, 51, 90, 68, 85, 122, 77, 122, 73, 120, 90, 68, 86, 104,
            78, 68, 108, 106, 90, 71, 82, 105, 78, 84, 99, 50, 89, 50, 73, 50, 79, 71, 78, 108, 89,
            84, 99, 119, 77, 109, 69, 50, 78, 109, 90, 109, 78, 109, 90, 109, 78, 71, 81, 52, 79,
            71, 78, 107, 79, 87, 90, 108, 90, 106, 77, 48, 90, 106, 99, 49, 77, 84, 78, 104, 78,
            122, 90, 107, 90, 106, 104, 109, 78, 122, 65, 52, 90, 109, 90, 107, 77, 68, 66, 105,
            90, 87, 73, 51, 89, 106, 65, 119, 78, 109, 73, 52, 90, 68, 89, 48, 79, 84, 85, 119, 78,
            122, 108, 107, 90, 84, 104, 105, 90, 84, 65, 122, 90, 71, 89, 51, 77, 68, 85, 49, 78,
            106, 82, 109, 78, 68, 66, 106, 77, 50, 74, 106, 89, 87, 69, 49, 77, 87, 85, 50, 77, 71,
            69, 51, 77, 68, 66, 105, 79, 71, 77, 122, 89, 109, 90, 108, 89, 122, 69, 119, 77, 68,
            65, 119, 77, 68, 65, 119, 77, 68, 65, 119, 77, 68, 65, 119, 77, 68, 73, 119, 78, 68,
            104, 104, 78, 106, 90, 108, 89, 84, 100, 104, 78, 84, 69, 53, 78, 109, 73, 52, 77, 109,
            69, 122, 89, 122, 85, 52, 89, 109, 82, 107, 90, 71, 73, 50, 77, 84, 65, 48, 77, 87, 73,
            119, 89, 122, 89, 52, 77, 122, 103, 119, 77, 68, 66, 107, 90, 71, 89, 48, 90, 84, 104,
            105, 90, 106, 81, 51, 78, 109, 69, 52, 90, 87, 73, 122, 78, 87, 69, 49, 90, 84, 99,
            121, 77, 87, 82, 109, 77, 109, 90, 104, 77, 122, 108, 106, 78, 106, 70, 106, 78, 68,
            89, 121, 89, 84, 70, 107, 78, 122, 86, 108, 89, 50, 78, 108, 77, 106, 65, 48, 77, 109,
            82, 106, 78, 122, 103, 49, 73, 105, 119, 105, 90, 71, 86, 106, 99, 110, 108, 119, 100,
            71, 108, 118, 98, 107, 78, 118, 98, 109, 82, 112, 100, 71, 108, 118, 98, 110, 77, 105,
            79, 105, 74, 88, 77, 51, 78, 112, 87, 84, 73, 53, 100, 87, 82, 73, 83, 109, 104, 90,
            77, 49, 74, 67, 87, 107, 100, 83, 101, 86, 112, 89, 84, 110, 112, 74, 97, 109, 57, 112,
            83, 87, 108, 51, 97, 87, 77, 122, 85, 109, 104, 105, 98, 86, 74, 111, 89, 50, 49, 83,
            82, 71, 73, 121, 78, 84, 66, 106, 98, 85, 90, 113, 90, 69, 90, 83, 78, 87, 78, 72, 86,
            87, 108, 80, 97, 85, 108, 112, 84, 69, 78, 75, 97, 109, 70, 72, 82, 110, 66, 105, 97,
            85, 107, 50, 83, 87, 49, 87, 77, 71, 70, 72, 86, 110, 108, 97, 87, 70, 90, 48, 83, 87,
            108, 51, 97, 87, 74, 88, 86, 106, 66, 104, 82, 122, 108, 114, 83, 87, 112, 118, 97, 85,
            108, 112, 100, 50, 108, 106, 82, 48, 90, 53, 87, 86, 99, 120, 98, 71, 82, 72, 86, 110,
            108, 106, 101, 85, 107, 50, 86, 51, 108, 74, 78, 109, 82, 89, 84, 109, 120, 106, 97,
            48, 90, 114, 87, 107, 104, 75, 98, 71, 77, 122, 84, 87, 108, 89, 85, 51, 100, 112, 89,
            50, 49, 87, 77, 71, 82, 89, 83, 110, 86, 87, 98, 85, 90, 122, 90, 70, 100, 87, 86, 86,
            112, 89, 84, 106, 66, 74, 97, 110, 65, 51, 83, 87, 49, 79, 100, 109, 74, 89, 81, 109,
            104, 106, 98, 85, 89, 119, 89, 106, 78, 74, 97, 85, 57, 112, 83, 84, 108, 74, 97, 88,
            100, 112, 90, 71, 49, 71, 99, 50, 82, 88, 86, 87, 108, 80, 97, 85, 108, 51, 90, 85, 82,
            78, 101, 69, 49, 116, 86, 107, 74, 80, 82, 70, 86, 53, 84, 110, 112, 74, 77, 108, 74,
            85, 84, 107, 74, 80, 86, 49, 107, 121, 84, 88, 112, 79, 81, 107, 49, 69, 84, 84, 78,
            79, 77, 107, 49, 51, 87, 108, 100, 70, 78, 69, 57, 69, 83, 88, 100, 80, 82, 70, 112,
            114, 84, 109, 112, 90, 77, 107, 53, 113, 87, 87, 108, 109, 87, 68, 66, 122, 90, 88,
            108, 75, 100, 109, 78, 72, 86, 110, 108, 90, 87, 70, 74, 50, 89, 50, 108, 74, 78, 107,
            108, 116, 82, 110, 86, 97, 81, 48, 111, 53, 84, 69, 104, 122, 97, 86, 107, 121, 79, 88,
            86, 107, 83, 69, 112, 111, 87, 84, 78, 83, 81, 108, 112, 72, 85, 110, 108, 97, 87, 69,
            53, 54, 83, 87, 112, 118, 97, 85, 108, 112, 100, 50, 108, 106, 77, 49, 74, 111, 89,
            109, 49, 83, 97, 71, 78, 116, 85, 107, 82, 105, 77, 106, 85, 119, 89, 50, 49, 71, 97,
            109, 82, 71, 85, 106, 86, 106, 82, 49, 86, 112, 84, 50, 108, 75, 86, 70, 78, 87, 90,
            69, 90, 74, 97, 88, 100, 112, 87, 84, 74, 111, 97, 71, 70, 88, 78, 71, 108, 80, 97, 85,
            112, 115, 90, 69, 100, 111, 98, 71, 78, 116, 86, 106, 70, 105, 85, 48, 108, 122, 83,
            87, 48, 120, 98, 71, 82, 72, 97, 72, 90, 97, 81, 48, 107, 50, 83, 87, 108, 74, 99, 48,
            108, 117, 81, 109, 104, 106, 98, 85, 90, 48, 87, 108, 104, 83, 98, 71, 78, 117, 84, 87,
            108, 80, 98, 72, 78, 112, 84, 50, 53, 75, 98, 71, 77, 121, 79, 84, 70, 106, 98, 85, 53,
            115, 89, 51, 108, 75, 90, 69, 120, 68, 83, 110, 108, 97, 87, 70, 73, 120, 89, 50, 48,
            49, 86, 49, 108, 88, 101, 68, 70, 97, 86, 108, 74, 115, 89, 122, 78, 82, 97, 85, 57,
            117, 99, 50, 108, 90, 77, 106, 108, 48, 89, 48, 100, 71, 101, 86, 108, 89, 85, 110, 90,
            106, 97, 85, 107, 50, 83, 87, 49, 79, 100, 109, 74, 117, 85, 109, 104, 104, 86, 122,
            86, 54, 83, 87, 108, 51, 97, 87, 82, 116, 82, 110, 78, 107, 86, 49, 86, 112, 84, 50,
            108, 75, 97, 108, 112, 89, 83, 109, 104, 105, 86, 50, 120, 113, 84, 50, 107, 52, 100,
            107, 116, 113, 79, 88, 82, 105, 77, 108, 74, 115, 89, 107, 81, 120, 99, 109, 70, 117,
            99, 72, 78, 79, 98, 87, 103, 121, 87, 109, 53, 75, 97, 87, 82, 54, 87, 109, 112, 79,
            87, 69, 90, 114, 90, 87, 53, 107, 99, 69, 57, 88, 86, 110, 112, 108, 83, 70, 111, 119,
            84, 86, 104, 90, 77, 87, 74, 89, 85, 106, 66, 79, 77, 106, 108, 114, 84, 106, 74, 111,
            97, 85, 49, 113, 97, 122, 66, 79, 101, 108, 108, 53, 84, 107, 99, 120, 100, 85, 53, 73,
            86, 88, 100, 106, 98, 84, 70, 52, 84, 86, 104, 75, 98, 48, 57, 88, 82, 110, 86, 104,
            98, 85, 53, 49, 90, 85, 104, 110, 97, 87, 90, 89, 77, 72, 78, 108, 101, 85, 112, 50,
            89, 48, 100, 87, 101, 86, 108, 89, 85, 110, 90, 106, 97, 85, 107, 50, 83, 87, 49, 71,
            100, 86, 112, 68, 83, 106, 108, 77, 83, 72, 78, 112, 87, 84, 73, 53, 100, 87, 82, 73,
            83, 109, 104, 90, 77, 49, 74, 67, 87, 107, 100, 83, 101, 86, 112, 89, 84, 110, 112, 74,
            97, 109, 57, 112, 83, 87, 108, 51, 97, 87, 77, 122, 85, 109, 104, 105, 98, 86, 74, 111,
            89, 50, 49, 83, 82, 71, 73, 121, 78, 84, 66, 106, 98, 85, 90, 113, 90, 69, 90, 83, 78,
            87, 78, 72, 86, 87, 108, 80, 97, 85, 112, 85, 85, 49, 90, 107, 82, 107, 108, 112, 100,
            50, 108, 90, 77, 109, 104, 111, 89, 86, 99, 48, 97, 85, 57, 112, 83, 109, 120, 107, 82,
            50, 104, 115, 89, 50, 49, 87, 77, 87, 74, 84, 83, 88, 78, 74, 98, 84, 70, 115, 90, 69,
            100, 111, 100, 108, 112, 68, 83, 84, 90, 74, 97, 85, 108, 122, 83, 87, 53, 67, 97, 71,
            78, 116, 82, 110, 82, 97, 87, 70, 74, 115, 89, 50, 53, 78, 97, 85, 57, 115, 99, 50,
            108, 80, 98, 107, 112, 115, 89, 122, 73, 53, 77, 87, 78, 116, 84, 109, 120, 106, 101,
            85, 112, 107, 84, 69, 78, 75, 101, 86, 112, 89, 85, 106, 70, 106, 98, 84, 86, 88, 87,
            86, 100, 52, 77, 86, 112, 87, 85, 109, 120, 106, 77, 49, 70, 112, 84, 50, 53, 122, 97,
            86, 107, 121, 79, 88, 82, 106, 82, 48, 90, 53, 87, 86, 104, 83, 100, 109, 78, 112, 83,
            84, 90, 74, 98, 85, 53, 50, 89, 109, 53, 83, 97, 71, 70, 88, 78, 88, 112, 74, 97, 88,
            100, 112, 90, 71, 49, 71, 99, 50, 82, 88, 86, 87, 108, 80, 97, 85, 112, 113, 87, 108,
            104, 75, 97, 71, 74, 88, 98, 71, 112, 80, 97, 84, 104, 50, 83, 50, 111, 53, 100, 71,
            73, 121, 85, 109, 120, 105, 82, 68, 70, 121, 89, 87, 53, 119, 99, 48, 53, 116, 97, 68,
            74, 97, 98, 107, 112, 112, 90, 72, 112, 97, 97, 107, 53, 116, 82, 109, 116, 79, 77, 50,
            120, 114, 89, 109, 112, 67, 98, 50, 70, 85, 85, 106, 74, 107, 82, 48, 90, 48, 90, 85,
            82, 75, 77, 107, 53, 113, 83, 88, 100, 104, 82, 49, 74, 117, 90, 70, 82, 97, 99, 50,
            74, 73, 82, 84, 66, 80, 86, 50, 100, 53, 84, 48, 104, 75, 98, 86, 112, 69, 87, 109,
            112, 106, 101, 107, 70, 53, 87, 110, 112, 79, 97, 109, 74, 88, 78, 68, 86, 108, 98, 85,
            86, 112, 90, 108, 103, 119, 99, 50, 86, 53, 83, 110, 90, 106, 82, 49, 90, 53, 87, 86,
            104, 83, 100, 109, 78, 112, 83, 84, 90, 74, 98, 85, 90, 49, 87, 107, 78, 75, 79, 85,
            120, 73, 99, 50, 108, 90, 77, 106, 108, 49, 90, 69, 104, 75, 97, 70, 107, 122, 85, 107,
            74, 97, 82, 49, 74, 53, 87, 108, 104, 79, 101, 107, 108, 113, 98, 50, 108, 74, 97, 88,
            100, 112, 89, 122, 78, 83, 97, 71, 74, 116, 85, 109, 104, 106, 98, 86, 74, 69, 89, 106,
            73, 49, 77, 71, 78, 116, 82, 109, 112, 107, 82, 108, 73, 49, 89, 48, 100, 86, 97, 85,
            57, 112, 83, 108, 82, 84, 86, 109, 82, 71, 83, 87, 108, 51, 97, 86, 107, 121, 97, 71,
            104, 104, 86, 122, 82, 112, 84, 50, 108, 75, 98, 71, 82, 72, 97, 71, 120, 106, 98, 86,
            89, 120, 89, 108, 78, 74, 99, 48, 108, 116, 77, 87, 120, 107, 82, 50, 104, 50, 87, 107,
            78, 74, 78, 107, 108, 112, 83, 88, 78, 74, 98, 107, 74, 111, 89, 50, 49, 71, 100, 70,
            112, 89, 85, 109, 120, 106, 98, 107, 49, 112, 84, 50, 120, 122, 97, 85, 57, 117, 83,
            109, 120, 106, 77, 106, 107, 120, 89, 50, 49, 79, 98, 71, 78, 53, 83, 109, 82, 77, 81,
            48, 112, 53, 87, 108, 104, 83, 77, 87, 78, 116, 78, 86, 100, 90, 86, 51, 103, 120, 87,
            108, 90, 83, 98, 71, 77, 122, 85, 87, 108, 80, 98, 110, 78, 112, 87, 84, 73, 53, 100,
            71, 78, 72, 82, 110, 108, 90, 87, 70, 74, 50, 89, 50, 108, 74, 78, 107, 108, 116, 84,
            110, 90, 105, 98, 108, 74, 111, 89, 86, 99, 49, 101, 107, 108, 112, 100, 50, 108, 107,
            98, 85, 90, 122, 90, 70, 100, 86, 97, 85, 57, 112, 83, 109, 112, 97, 87, 69, 112, 111,
            89, 108, 100, 115, 97, 107, 57, 112, 79, 72, 90, 76, 97, 106, 108, 48, 89, 106, 74, 83,
            98, 71, 74, 69, 77, 88, 74, 104, 98, 110, 66, 122, 84, 109, 49, 111, 77, 108, 112, 117,
            83, 109, 108, 107, 101, 108, 112, 113, 84, 110, 112, 90, 101, 109, 82, 88, 83, 109,
            116, 104, 82, 122, 107, 122, 90, 87, 49, 71, 100, 107, 49, 72, 77, 68, 66, 108, 87, 69,
            69, 48, 84, 107, 100, 79, 78, 71, 86, 116, 83, 109, 49, 105, 98, 88, 104, 118, 84, 107,
            100, 111, 97, 50, 70, 85, 86, 109, 104, 105, 83, 69, 90, 50, 84, 107, 104, 115, 101,
            86, 112, 88, 83, 110, 82, 90, 101, 107, 74, 52, 89, 48, 100, 119, 97, 50, 70, 85, 86,
            87, 108, 109, 87, 68, 70, 107, 73, 105, 119, 105, 90, 87, 53, 106, 99, 110, 108, 119,
            100, 71, 86, 107, 73, 106, 111, 105, 81, 85, 82, 89, 99, 87, 99, 51, 87, 108, 100, 75,
            86, 69, 112, 53, 83, 88, 82, 89, 90, 106, 70, 105, 77, 50, 116, 79, 77, 88, 112, 89,
            98, 84, 82, 66, 77, 86, 104, 66, 82, 85, 119, 52, 87, 69, 116, 84, 101, 84, 86, 116,
            87, 88, 70, 116, 90, 50, 82, 117, 83, 50, 57, 69, 90, 107, 78, 105, 86, 51, 100, 104,
            84, 88, 100, 53, 100, 48, 90, 84, 101, 85, 100, 109, 78, 108, 90, 86, 86, 70, 104, 114,
            101, 88, 70, 122, 101, 109, 108, 52, 85, 107, 74, 72, 76, 87, 119, 122, 79, 71, 120,
            112, 84, 86, 86, 102, 78, 68, 74, 119, 90, 87, 73, 48, 77, 50, 90, 48, 89, 107, 100,
            83, 98, 50, 78, 110, 84, 87, 70, 77, 77, 110, 86, 76, 77, 106, 90, 116, 85, 88, 73,
            122, 77, 86, 82, 67, 99, 71, 82, 90, 81, 86, 108, 52, 82, 108, 108, 86, 86, 122, 85,
            105, 102, 81, 105, 99, 114, 101, 97, 116, 101, 100, 65, 116, 120, 24, 50, 48, 50, 51,
            45, 48, 54, 45, 49, 49, 84, 49, 49, 58, 50, 52, 58, 50, 51, 46, 48, 50, 57, 90, 105,
            117, 112, 100, 97, 116, 101, 100, 65, 116, 120, 24, 50, 48, 50, 51, 45, 48, 54, 45, 49,
            49, 84, 49, 49, 58, 50, 52, 58, 50, 51, 46, 48, 50, 57, 90, 106, 97, 112, 112, 86, 101,
            114, 115, 105, 111, 110, 101, 48, 46, 50, 46, 48, 106, 102, 111, 108, 100, 101, 114,
            84, 121, 112, 101, 1, 112, 99, 111, 110, 116, 101, 110, 116, 70, 111, 108, 100, 101,
            114, 73, 100, 115, 129, 100, 116, 101, 109, 112, 102, 104, 101, 97, 100, 101, 114, 163,
            101, 109, 111, 100, 101, 108, 88, 40, 206, 1, 2, 1, 133, 1, 18, 32, 28, 223, 70, 218,
            119, 176, 192, 189, 112, 60, 128, 40, 16, 35, 243, 58, 191, 55, 101, 221, 228, 130, 79,
            204, 65, 40, 84, 172, 175, 200, 147, 53, 102, 117, 110, 105, 113, 117, 101, 76, 43,
            109, 113, 26, 153, 48, 39, 107, 155, 128, 50, 162, 107, 99, 111, 110, 116, 114, 111,
            108, 108, 101, 114, 115, 129, 120, 59, 100, 105, 100, 58, 112, 107, 104, 58, 101, 105,
            112, 49, 53, 53, 58, 49, 58, 48, 120, 51, 49, 50, 101, 65, 56, 53, 50, 55, 50, 54, 69,
            51, 65, 57, 102, 54, 51, 51, 65, 48, 51, 55, 55, 99, 48, 101, 97, 56, 56, 50, 48, 56,
            54, 100, 54, 54, 54, 54, 54,
        ];

        // Act
        let result = IpldDecodeFrom::<Payload>::decode(&data);

        // Assert
        assert!(result.is_ok());
        let payload = result.unwrap();
        assert_eq!(payload.prev, None);
        assert_eq!(payload.id, None);
        assert!(payload.data.is_some());
        let data = payload.data;
        assert!(data.is_some());
    }

    #[test]
    fn test_decode_cacao() {
        let data = vec![
            163, 97, 104, 161, 97, 116, 103, 101, 105, 112, 52, 51, 54, 49, 97, 112, 169, 99, 97,
            117, 100, 120, 56, 100, 105, 100, 58, 107, 101, 121, 58, 122, 54, 77, 107, 116, 68, 86,
            68, 85, 104, 69, 97, 117, 76, 98, 69, 69, 90, 77, 83, 65, 116, 82, 49, 55, 55, 100, 68,
            121, 99, 100, 111, 122, 99, 120, 82, 102, 119, 80, 113, 84, 50, 106, 81, 86, 74, 85,
            55, 99, 101, 120, 112, 120, 24, 50, 48, 50, 51, 45, 49, 48, 45, 49, 52, 84, 48, 55, 58,
            50, 57, 58, 50, 51, 46, 49, 48, 50, 90, 99, 105, 97, 116, 120, 24, 50, 48, 50, 51, 45,
            49, 48, 45, 48, 55, 84, 48, 55, 58, 50, 57, 58, 50, 51, 46, 49, 48, 50, 90, 99, 105,
            115, 115, 120, 59, 100, 105, 100, 58, 112, 107, 104, 58, 101, 105, 112, 49, 53, 53, 58,
            49, 58, 48, 120, 53, 57, 49, 53, 101, 50, 57, 51, 56, 50, 51, 70, 67, 97, 56, 52, 48,
            99, 57, 51, 69, 68, 50, 69, 49, 69, 53, 66, 52, 100, 102, 51, 50, 100, 54, 57, 57, 57,
            57, 57, 101, 110, 111, 110, 99, 101, 110, 68, 100, 110, 55, 108, 83, 99, 51, 118, 81,
            84, 119, 113, 118, 102, 100, 111, 109, 97, 105, 110, 120, 32, 99, 101, 107, 112, 102,
            110, 107, 108, 99, 105, 102, 105, 111, 109, 103, 101, 111, 103, 98, 109, 107, 110, 110,
            109, 99, 103, 98, 107, 100, 112, 105, 109, 103, 118, 101, 114, 115, 105, 111, 110, 97,
            49, 105, 114, 101, 115, 111, 117, 114, 99, 101, 115, 138, 120, 81, 99, 101, 114, 97,
            109, 105, 99, 58, 47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108, 54,
            104, 118, 102, 114, 98, 119, 54, 99, 56, 115, 111, 103, 99, 99, 52, 51, 56, 102, 103,
            103, 115, 117, 110, 121, 98, 117, 113, 54, 113, 57, 101, 99, 120, 111, 97, 111, 122,
            99, 120, 101, 56, 113, 108, 106, 107, 56, 119, 117, 51, 117, 113, 117, 51, 57, 52, 117,
            120, 55, 120, 81, 99, 101, 114, 97, 109, 105, 99, 58, 47, 47, 42, 63, 109, 111, 100,
            101, 108, 61, 107, 106, 122, 108, 54, 104, 118, 102, 114, 98, 119, 54, 99, 97, 116,
            101, 107, 51, 54, 104, 51, 112, 101, 112, 48, 57, 107, 57, 103, 121, 109, 102, 110,
            108, 97, 57, 107, 54, 111, 106, 108, 103, 114, 109, 119, 106, 111, 103, 118, 106, 113,
            103, 56, 113, 51, 122, 112, 121, 98, 108, 49, 121, 117, 120, 81, 99, 101, 114, 97, 109,
            105, 99, 58, 47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108, 54, 104,
            118, 102, 114, 98, 119, 54, 99, 55, 120, 108, 116, 104, 122, 120, 57, 100, 105, 121,
            54, 107, 51, 114, 51, 115, 48, 120, 97, 102, 56, 104, 55, 52, 110, 103, 120, 104, 110,
            99, 103, 106, 119, 121, 101, 112, 108, 53, 56, 112, 107, 97, 49, 53, 120, 57, 121, 104,
            99, 120, 81, 99, 101, 114, 97, 109, 105, 99, 58, 47, 47, 42, 63, 109, 111, 100, 101,
            108, 61, 107, 106, 122, 108, 54, 104, 118, 102, 114, 98, 119, 54, 99, 56, 54, 49, 99,
            122, 118, 100, 115, 108, 101, 100, 51, 121, 108, 115, 97, 57, 57, 55, 55, 105, 55, 114,
            108, 111, 119, 121, 99, 57, 108, 55, 106, 112, 103, 54, 101, 49, 104, 106, 119, 104,
            57, 102, 101, 102, 108, 54, 98, 115, 117, 120, 81, 99, 101, 114, 97, 109, 105, 99, 58,
            47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108, 54, 104, 118, 102,
            114, 98, 119, 54, 99, 98, 52, 109, 115, 100, 56, 56, 105, 56, 109, 108, 106, 122, 121,
            112, 51, 97, 122, 119, 48, 57, 120, 50, 54, 118, 51, 107, 106, 111, 106, 101, 105, 116,
            98, 101, 120, 49, 56, 49, 101, 102, 105, 57, 52, 103, 53, 56, 101, 108, 102, 120, 81,
            99, 101, 114, 97, 109, 105, 99, 58, 47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107,
            106, 122, 108, 54, 104, 118, 102, 114, 98, 119, 54, 99, 55, 103, 117, 56, 56, 103, 54,
            54, 122, 50, 56, 110, 56, 49, 108, 99, 112, 98, 103, 54, 104, 117, 50, 116, 56, 112,
            117, 50, 112, 117, 105, 48, 115, 102, 110, 112, 118, 115, 114, 104, 113, 110, 51, 107,
            120, 104, 57, 120, 97, 105, 120, 81, 99, 101, 114, 97, 109, 105, 99, 58, 47, 47, 42,
            63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108, 54, 104, 118, 102, 114, 98, 119,
            54, 99, 97, 119, 114, 108, 55, 102, 55, 54, 55, 98, 54, 99, 122, 52, 56, 100, 110, 48,
            101, 102, 114, 57, 119, 102, 116, 120, 57, 116, 57, 106, 101, 108, 119, 57, 116, 98,
            49, 111, 116, 120, 122, 55, 53, 50, 106, 104, 56, 54, 107, 110, 120, 81, 99, 101, 114,
            97, 109, 105, 99, 58, 47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108,
            54, 104, 118, 102, 114, 98, 119, 54, 99, 56, 54, 103, 116, 57, 106, 52, 49, 53, 121,
            119, 50, 120, 56, 115, 116, 109, 107, 111, 116, 99, 114, 122, 112, 101, 117, 116, 114,
            98, 107, 112, 52, 50, 105, 52, 122, 57, 48, 103, 112, 53, 105, 98, 112, 116, 122, 52,
            115, 115, 111, 120, 81, 99, 101, 114, 97, 109, 105, 99, 58, 47, 47, 42, 63, 109, 111,
            100, 101, 108, 61, 107, 106, 122, 108, 54, 104, 118, 102, 114, 98, 119, 54, 99, 54,
            118, 98, 54, 52, 119, 105, 56, 56, 117, 98, 52, 55, 103, 98, 109, 99, 104, 56, 50, 119,
            99, 112, 98, 109, 101, 53, 49, 104, 121, 109, 52, 115, 57, 113, 98, 112, 50, 117, 107,
            97, 99, 48, 121, 116, 104, 122, 98, 106, 57, 120, 81, 99, 101, 114, 97, 109, 105, 99,
            58, 47, 47, 42, 63, 109, 111, 100, 101, 108, 61, 107, 106, 122, 108, 54, 104, 118, 102,
            114, 98, 119, 54, 99, 97, 103, 116, 54, 57, 52, 105, 105, 109, 50, 119, 117, 101, 99,
            117, 55, 101, 117, 109, 101, 100, 115, 55, 113, 100, 48, 112, 54, 117, 122, 109, 56,
            100, 110, 113, 115, 113, 54, 57, 108, 108, 55, 107, 97, 99, 109, 48, 53, 103, 117, 105,
            115, 116, 97, 116, 101, 109, 101, 110, 116, 120, 49, 71, 105, 118, 101, 32, 116, 104,
            105, 115, 32, 97, 112, 112, 108, 105, 99, 97, 116, 105, 111, 110, 32, 97, 99, 99, 101,
            115, 115, 32, 116, 111, 32, 115, 111, 109, 101, 32, 111, 102, 32, 121, 111, 117, 114,
            32, 100, 97, 116, 97, 97, 115, 162, 97, 115, 120, 132, 48, 120, 102, 100, 50, 52, 102,
            101, 100, 53, 48, 52, 50, 97, 101, 50, 55, 99, 98, 102, 53, 54, 101, 49, 55, 97, 102,
            54, 98, 102, 102, 55, 97, 52, 52, 48, 101, 54, 100, 49, 54, 53, 52, 102, 98, 55, 56,
            102, 101, 100, 56, 100, 51, 98, 98, 55, 98, 55, 100, 99, 57, 52, 97, 50, 97, 99, 50,
            102, 53, 50, 101, 55, 51, 97, 48, 48, 55, 101, 100, 56, 101, 48, 49, 49, 55, 48, 54,
            48, 102, 50, 55, 54, 99, 53, 57, 54, 49, 51, 97, 56, 100, 54, 57, 98, 56, 54, 56, 50,
            53, 50, 101, 98, 54, 98, 49, 97, 52, 49, 97, 55, 100, 97, 100, 101, 97, 101, 51, 54,
            55, 51, 49, 98, 97, 116, 102, 101, 105, 112, 49, 57, 49,
        ];
        let node: Ipld = DagCborCodec.decode(&data).unwrap();
        let cacao = libipld::serde::from_ipld::<CACAO>(node);
        assert!(cacao.is_ok());
    }
}
