use int_enum::IntEnum;

#[derive(Debug)]
pub struct StreamIdType(ceramic_core::StreamIdType);

impl From<ceramic_core::StreamIdType> for StreamIdType {
	fn from(value: ceramic_core::StreamIdType) -> Self {
		return StreamIdType(value);
	}
}

impl TryFrom<u64> for StreamIdType {
	type Error = anyhow::Error;

	fn try_from(value: u64) -> Result<Self, Self::Error> {
		Ok(StreamIdType(ceramic_core::StreamIdType::from_int(value)?))
	}
}

impl Into<ceramic_core::StreamIdType> for StreamIdType {
	fn into(self) -> ceramic_core::StreamIdType {
		self.0
	}
}

impl Into<u64> for StreamIdType {
	fn into(self) -> u64 {
		todo!()
	}
}
