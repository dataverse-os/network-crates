use std::str::FromStr;

use anyhow::Result;
use ceramic_core::{Base64String, Base64UrlString};
use dag_jose::{DagJoseCodec, JsonWebSignature};
use libipld::multihash::{Code, MultihashDigest};
use libipld::prelude::Codec;
use libipld::{Cid, Ipld};

use super::IpldAs;

pub trait ToCid {
	fn cid(&self) -> anyhow::Result<Cid>;
	fn to_vec(&self) -> anyhow::Result<Vec<u8>>;
}

impl ToCid for ceramic_core::Jws {
	fn cid(&self) -> anyhow::Result<Cid> {
		let jws: JsonWebSignature = TryIntoJwsSignature::try_into(self)?;
		jws.cid()
	}

	fn to_vec(&self) -> anyhow::Result<Vec<u8>> {
		let jws: JsonWebSignature = TryIntoJwsSignature::try_into(self)?;
		jws.to_vec()
	}
}

impl ToCid for JsonWebSignature {
	fn cid(&self) -> anyhow::Result<Cid> {
		Ok(Cid::new_v1(
			0x85,
			Code::Sha2_256.digest(DagJoseCodec.encode(&self)?.as_ref()),
		))
	}

	fn to_vec(&self) -> anyhow::Result<Vec<u8>> {
		Ok(DagJoseCodec.encode(&self)?)
	}
}

pub trait TryIntoJwsSignature {
	fn try_into(&self) -> anyhow::Result<JsonWebSignature>;
}

impl TryIntoJwsSignature for ceramic_core::Jws {
	fn try_into(&self) -> anyhow::Result<JsonWebSignature> {
		let link = match self.link.clone() {
			Some(val) => val,
			None => anyhow::bail!("JWS does not have a link"),
		};
		let signatures = self
			.signatures
			.iter()
			.map(|x| dag_jose::Signature {
				header: Default::default(),
				protected: x.protected.as_ref().map(|s| s.to_string()),
				signature: x.signature.to_string(),
			})
			.collect();

		Ok(JsonWebSignature {
			payload: self.payload.to_string(),
			signatures,
			link: Cid::from_str(link.as_ref())?,
		})
	}
}

pub trait IpldAsJws {
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
