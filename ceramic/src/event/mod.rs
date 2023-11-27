mod anchor;
pub mod commit;
mod ipld;
pub mod jws;
mod signed;
mod verify;

use crate::stream::{LogType, StreamState};
use anyhow::Result;
use ceramic_http_client::api::StateLog;
use libipld::prelude::Codec;
use libipld::{cbor::DagCborCodec, cid::Cid};
use serde::{Deserialize, Serialize};

pub use self::anchor::*;
pub use self::ipld::*;
use self::jws::ToCid;
pub use self::signed::*;
pub use self::verify::*;

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
                    proof_block: None,
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

trait StreamStateApplyer {
    fn apply_to(&self, stream_state: &mut StreamState) -> anyhow::Result<()>;
}

impl StreamStateApplyer for EventValue {
    fn apply_to(&self, stream_state: &mut StreamState) -> anyhow::Result<()> {
        if let Self::Signed(signed) = self {
            return signed.apply_to(stream_state);
        };
        Ok(())
    }
}

impl EventValue {
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use libipld::Ipld;

    use crate::cacao::CACAO;

    use super::*;

    #[test]
    fn decode_cacao_cap() -> anyhow::Result<()> {
        let genesis = crate::commit::example::genesis();
        let signed: SignedValue = genesis.genesis.try_into().unwrap();

        let cap = signed.cap();
        assert!(cap.is_ok());

        let cacao_cid = signed.cacao_link();
        assert!(cacao_cid.is_ok());
        assert_eq!(cap.unwrap(), cacao_cid.unwrap());

        Ok(())
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
            proof_block: None,
        };

        assert_eq!(result, expected);
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
