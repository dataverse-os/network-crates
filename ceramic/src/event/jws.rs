use std::str::FromStr;

use ceramic_core::MultiBase32String;
use dag_jose::{DagJoseCodec, JsonWebSignature};
use libipld::multihash::{Code, MultihashDigest};
use libipld::prelude::Codec;
use libipld::Cid;

pub trait ToCid {
	fn cid(&self) -> anyhow::Result<Cid>;
	fn to_vec(&self) -> anyhow::Result<Vec<u8>>;
}

impl ToCid for ceramic_core::Jws {
	fn cid(&self) -> anyhow::Result<Cid> {
		let jws: Jws = self.into();
		let jws: JsonWebSignature = jws.try_into()?;
		jws.cid()
	}

	fn to_vec(&self) -> anyhow::Result<Vec<u8>> {
		let jws: Jws = self.into();
		let jws: JsonWebSignature = jws.try_into()?;
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

#[derive(Debug)]
pub(crate) struct Jws(pub ceramic_core::Jws);

pub fn clone_jws(jws: &ceramic_core::Jws) -> ceramic_core::Jws {
	ceramic_core::Jws {
		link: jws.link.clone(),
		payload: jws.payload.clone(),
		signatures: jws
			.signatures
			.iter()
			.map(|sig| ceramic_core::JwsSignature {
				protected: sig.protected.clone(),
				signature: sig.signature.clone(),
			})
			.collect::<Vec<_>>(),
	}
}

impl Clone for Jws {
	fn clone(&self) -> Self {
		let Self(jws) = self;
		Self(clone_jws(jws))
	}
}

impl From<&ceramic_core::Jws> for Jws {
	fn from(value: &ceramic_core::Jws) -> Self {
		Self(clone_jws(value))
	}
}

impl Into<ceramic_core::Jws> for Jws {
	fn into(self) -> ceramic_core::Jws {
		self.0
	}
}

impl TryFrom<Vec<u8>> for Jws {
	type Error = anyhow::Error;

	fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
		let jws = DagJoseCodec.decode::<JsonWebSignature>(&value)?;
		jws.try_into()
	}
}

impl TryFrom<JsonWebSignature> for Jws {
	type Error = anyhow::Error;

	fn try_from(value: JsonWebSignature) -> Result<Self, Self::Error> {
		let link = MultiBase32String::try_from(&value.link)?;
		let jws = ceramic_core::Jws {
			link: Some(link),
			payload: value.payload.into(),
			signatures: value
				.signatures
				.into_iter()
				.map(|sig| ceramic_core::JwsSignature {
					protected: sig.protected.map(Into::into),
					signature: sig.signature.into(),
				})
				.collect::<Vec<_>>(),
		};
		Ok(Jws(jws))
	}
}

impl TryInto<JsonWebSignature> for Jws {
	type Error = anyhow::Error;

	fn try_into(self) -> Result<JsonWebSignature, Self::Error> {
		let Self(jws) = self;
		let link = match jws.link.clone() {
			Some(val) => val,
			None => anyhow::bail!("JWS does not have a link"),
		};
		let signatures = jws
			.signatures
			.iter()
			.map(|x| dag_jose::Signature {
				header: Default::default(),
				protected: x.protected.as_ref().map(|s| s.to_string()),
				signature: x.signature.to_string(),
			})
			.collect();

		Ok(JsonWebSignature {
			payload: jws.payload.to_string(),
			signatures,
			link: Cid::from_str(link.as_ref())?,
		})
	}
}

#[cfg(test)]
mod tests {
	use libipld::Ipld;

	use super::*;

	#[test]
	fn decode_encode_jws() {
		let data = vec![
			162, 103, 112, 97, 121, 108, 111, 97, 100, 88, 36, 1, 113, 18, 32, 4, 225, 213, 10, 14,
			128, 171, 9, 65, 34, 32, 28, 124, 24, 204, 111, 153, 134, 197, 242, 139, 192, 65, 70,
			243, 168, 166, 43, 72, 35, 107, 114, 106, 115, 105, 103, 110, 97, 116, 117, 114, 101,
			115, 129, 162, 105, 112, 114, 111, 116, 101, 99, 116, 101, 100, 88, 204, 123, 34, 97,
			108, 103, 34, 58, 34, 69, 100, 68, 83, 65, 34, 44, 34, 99, 97, 112, 34, 58, 34, 105,
			112, 102, 115, 58, 47, 47, 98, 97, 102, 121, 114, 101, 105, 101, 118, 99, 52, 103, 53,
			112, 107, 117, 113, 119, 119, 112, 104, 111, 106, 50, 102, 50, 100, 116, 53, 111, 100,
			52, 54, 52, 116, 55, 120, 115, 112, 105, 106, 99, 107, 121, 53, 110, 105, 51, 106, 111,
			107, 100, 99, 113, 53, 102, 100, 55, 97, 34, 44, 34, 107, 105, 100, 34, 58, 34, 100,
			105, 100, 58, 107, 101, 121, 58, 122, 54, 77, 107, 113, 107, 82, 110, 69, 119, 85, 66,
			99, 78, 66, 106, 110, 99, 104, 71, 65, 71, 67, 114, 117, 109, 84, 103, 109, 67, 75, 76,
			68, 67, 98, 102, 65, 84, 109, 87, 104, 84, 113, 54, 65, 105, 106, 109, 35, 122, 54, 77,
			107, 113, 107, 82, 110, 69, 119, 85, 66, 99, 78, 66, 106, 110, 99, 104, 71, 65, 71, 67,
			114, 117, 109, 84, 103, 109, 67, 75, 76, 68, 67, 98, 102, 65, 84, 109, 87, 104, 84,
			113, 54, 65, 105, 106, 109, 34, 125, 105, 115, 105, 103, 110, 97, 116, 117, 114, 101,
			88, 64, 238, 236, 173, 161, 246, 8, 88, 125, 162, 186, 97, 232, 132, 24, 78, 95, 32,
			180, 183, 197, 36, 180, 13, 83, 5, 20, 150, 69, 1, 75, 179, 42, 143, 129, 85, 204, 157,
			94, 141, 139, 254, 24, 128, 231, 239, 246, 131, 255, 9, 124, 112, 98, 250, 25, 84, 82,
			12, 129, 143, 125, 122, 17, 39, 11,
		];

		let node = DagJoseCodec.decode::<Ipld>(&data);
		assert!(node.is_ok());

		let jws = DagJoseCodec.decode::<JsonWebSignature>(&data);
		assert!(jws.is_ok());
		let jws = jws.unwrap();

		let jws = TryInto::<Jws>::try_into(jws);
		assert!(jws.is_ok());
		let jws = jws.unwrap();

		let encoded = TryInto::<JsonWebSignature>::try_into(jws);
		assert!(encoded.is_ok());

		let encoded = encoded.unwrap().to_vec();
		assert!(encoded.is_ok());
		assert_eq!(encoded.unwrap(), data);
	}
}
