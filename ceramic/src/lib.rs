pub mod did;
pub mod event;
pub mod http;
pub mod kubo;
pub mod network;
pub mod stream;

pub use ceramic_core::StreamId;
pub use cid::Cid;
pub use event::commit;
pub use event::{Event, EventValue, EventsLoader, EventsUploader};
use serde::{Deserialize, Serialize};
pub use stream::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ceramic {
	pub endpoint: String,
	pub network: network::Network,
}

impl Ceramic {
	pub async fn new(endpoint: &str) -> anyhow::Result<Self> {
		let network = http::Client::network(endpoint).await?;
		let endpoint = endpoint.into();
		Ok(Self { endpoint, network })
	}
}

macro_rules! impl_multi_base {
	($typname:ident, $base:expr) => {
		/// A string that is encoded with a multibase prefix
		#[derive(Clone, Debug, Deserialize, Serialize)]
		#[serde(transparent)]
		pub struct $typname(String);

		impl std::convert::TryFrom<&Cid> for $typname {
			type Error = anyhow::Error;

			fn try_from(v: &Cid) -> Result<Self, Self::Error> {
				let s = v.to_string_of_base($base)?;
				Ok(Self(s))
			}
		}

		impl std::convert::TryFrom<&StreamId> for $typname {
			type Error = anyhow::Error;

			fn try_from(v: &StreamId) -> Result<Self, Self::Error> {
				let v = v.to_vec()?;
				Ok(Self::from(v))
			}
		}

		impl AsRef<str> for $typname {
			fn as_ref(&self) -> &str {
				&self.0
			}
		}

		impl From<&[u8]> for $typname {
			fn from(value: &[u8]) -> Self {
				Self(multibase::encode($base, value))
			}
		}

		impl From<Vec<u8>> for $typname {
			fn from(value: Vec<u8>) -> Self {
				Self::from(value.as_slice())
			}
		}
	};
}

impl_multi_base!(MultiBase32String, multibase::Base::Base32Lower);
