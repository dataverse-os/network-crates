use std::{fmt::Formatter, io::Write};

use cid::Cid;
use int_enum::IntEnum;
use multibase::Base;
use serde::{Deserialize, Deserializer, Serialize};
use unsigned_varint::{decode, encode};

const STREAMID_CODEC: u64 = 206;

/// A stream id, which is a cid with a type
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamId {
	/// The type of the stream
	pub r#type: StreamIdType,
	/// Cid of the stream
	pub cid: Cid,
}

impl StreamId {
	/// Write the stream id to a writer
	pub fn write<W: Write>(&self, mut writer: W) -> anyhow::Result<()> {
		let mut buf = encode::u64_buffer();
		let v = encode::u64(STREAMID_CODEC, &mut buf);
		writer.write_all(v)?;
		let v = encode::u64(self.r#type.int_value(), &mut buf);
		writer.write_all(v)?;
		self.cid.write_bytes(&mut writer)?;
		Ok(())
	}

	/// Convert the stream id to a vector
	pub fn to_vec(&self) -> anyhow::Result<Vec<u8>> {
		// Use self.len() here when we have cid@0.10
		let buf = Vec::new();
		let mut writer = std::io::BufWriter::new(buf);
		self.write(&mut writer)?;
		Ok(writer.into_inner()?)
	}
}

impl std::str::FromStr for StreamId {
	type Err = anyhow::Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let (_, id) = multibase::decode(s)?;
		Self::try_from(id.as_slice())
	}
}

impl std::fmt::Display for StreamId {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		if let Ok(b) = self.to_vec() {
			let s = multibase::encode(Base::Base36Lower, b);
			write!(f, "{}", s)
		} else {
			Err(std::fmt::Error)
		}
	}
}

impl TryFrom<&[u8]> for StreamId {
	type Error = anyhow::Error;

	fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
		let (value, rest) = decode::u64(value)?;
		if value == STREAMID_CODEC {
			let (tpe, rest) = decode::u64(rest)?;
			let tpe = StreamIdType::from_int(tpe)?;
			let cid = Cid::read_bytes(std::io::BufReader::new(rest))?;
			Ok(StreamId { r#type: tpe, cid })
		} else {
			anyhow::bail!("Invalid StreamId, does not include StreamId Codec");
		}
	}
}

#[cfg(feature = "ceramic-core")]
impl From<ceramic_core::StreamId> for StreamId {
	fn from(value: ceramic_core::StreamId) -> Self {
		Self {
			r#type: value.r#type.into(),
			cid: value.cid,
		}
	}
}

#[cfg(feature = "ceramic-core")]
impl Into<ceramic_core::StreamId> for StreamId {
	fn into(self) -> ceramic_core::StreamId {
		ceramic_core::StreamId {
			r#type: self.r#type.into(),
			cid: self.cid,
		}
	}
}

impl Serialize for StreamId {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::ser::Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

impl<'de> Deserialize<'de> for StreamId {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_str(StreamIdVisitor)
	}
}

struct StreamIdVisitor;

impl<'de> serde::de::Visitor<'de> for StreamIdVisitor {
	type Value = StreamId;

	fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
		formatter.write_str("a multi base string")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		match multibase::decode(v) {
			Ok((_, v)) => StreamId::try_from(v.as_slice())
				.map_err(|e| serde::de::Error::custom(format!("{:?}", e))),
			Err(e) => Err(serde::de::Error::custom(format!("{:?}", e))),
		}
	}

	fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		self.visit_str(&v)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::str::FromStr;

	#[test]
	fn can_serialize_and_deserialize_correctly() {
		let orig = "kjzl6kcym7w8y7nzgytqayf6aro12zt0mm01n6ydjomyvvklcspx9kr6gpbwd09";
		let stream = StreamId::from_str(orig).unwrap();
		assert_eq!(stream.r#type, StreamIdType::ModelInstanceDocument);
		let s = stream.to_string();
		assert_eq!(&s, orig);
	}
}

/// Types of possible stream id's
/// Defined here:
/// https://cips.ceramic.network/tables/streamtypes.csv
#[repr(u64)]
#[derive(Copy, Clone, Debug, Eq, IntEnum, PartialEq)]
pub enum StreamIdType {
	/// A stream type representing a json document
	/// https://cips.ceramic.network/CIPs/cip-8
	Tile = 0,
	/// Link blockchain accounts to DIDs
	/// https://cips.ceramic.network/CIPs/cip-7
	Caip10Link = 1,
	/// Defines a schema shared by group of documents in ComposeDB
	/// https://github.com/ceramicnetwork/js-ceramic/tree/main/packages/stream-model
	Model = 2,
	/// Represents a json document in ComposeDB
	/// https://github.com/ceramicnetwork/js-ceramic/tree/main/packages/stream-model-instance
	ModelInstanceDocument = 3,
	/// A stream that is not meant to be loaded
	/// https://github.com/ceramicnetwork/js-ceramic/blob/main/packages/stream-model/src/model.ts#L163-L165
	Unloadable = 4,
	/// An event id encoded as a cip-124 EventID
	/// https://cips.ceramic.network/CIPs/cip-124
	EventId = 5,
}

#[cfg(feature = "ceramic-core")]
impl Into<ceramic_core::StreamIdType> for StreamIdType {
	fn into(self) -> ceramic_core::StreamIdType {
		match self {
			StreamIdType::Tile => ceramic_core::StreamIdType::Tile,
			StreamIdType::Caip10Link => ceramic_core::StreamIdType::Caip10Link,
			StreamIdType::Model => ceramic_core::StreamIdType::Model,
			StreamIdType::ModelInstanceDocument => {
				ceramic_core::StreamIdType::ModelInstanceDocument
			}
			StreamIdType::Unloadable => ceramic_core::StreamIdType::Unloadable,
			StreamIdType::EventId => ceramic_core::StreamIdType::EventId,
		}
	}
}

#[cfg(feature = "ceramic-core")]
impl From<ceramic_core::StreamIdType> for StreamIdType {
	fn from(value: ceramic_core::StreamIdType) -> Self {
		match value {
			ceramic_core::StreamIdType::Tile => StreamIdType::Tile,
			ceramic_core::StreamIdType::Caip10Link => StreamIdType::Caip10Link,
			ceramic_core::StreamIdType::Model => StreamIdType::Model,
			ceramic_core::StreamIdType::ModelInstanceDocument => {
				StreamIdType::ModelInstanceDocument
			}
			ceramic_core::StreamIdType::Unloadable => StreamIdType::Unloadable,
			ceramic_core::StreamIdType::EventId => StreamIdType::EventId,
		}
	}
}
