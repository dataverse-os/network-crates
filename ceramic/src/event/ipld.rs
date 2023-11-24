use anyhow::Result;
use ceramic_core::{Base64String, Base64UrlString};
use dag_jose::DagJoseCodec;
use libipld::prelude::Codec;
use libipld::{cid::Cid, Ipld};

use super::{Event, SignedValue};

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

pub trait IpldAs<T> {
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
