use ceramic_core::Base64String;
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::access_control::AccessControl;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFolder {
	pub folder_name: String,
	pub folder_type: FolderType,
	pub created_at: DateTime<Utc>,
	pub updated_at: DateTime<Utc>,
	pub fs_version: String,

	pub access_control: Base64String,
	pub content_folder_ids: Vec<String>,

	pub options: Option<Base64String>,
	pub deleted: Option<bool>,
	pub reserved: Option<String>,
}

impl IndexFolder {
	#[allow(dead_code)]
	pub fn options(&self) -> anyhow::Result<Option<FolderOptions>> {
		if let Some(options) = &self.options {
			Ok(serde_json::from_slice(options.to_vec()?.as_ref())?)
		} else {
			Ok(None)
		}
	}

	#[allow(dead_code)]
	pub fn access_control(&self) -> anyhow::Result<AccessControl> {
		Ok(serde_json::from_slice(&self.access_control.to_vec()?)?)
	}
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FolderOptions {
	pub folder_description: Option<String>,
	pub signals: Vec<Value>,
}

#[repr(u64)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, IntEnum)]
pub enum FolderType {
	PrivateFolderType = 0,
	UnionFolderType = 1,
}

#[cfg(test)]
mod tests {
	use super::*;
	use base64::{engine::general_purpose, Engine};
	use serde_json::json;

	#[test]
	fn test_decode_folder_options() {
		let encoded_data = "eyJzaWduYWxzIjpbeyJ0eXBlIjoyLCJpZCI6IjB4YmRiNmYwZmViYTMwM2RiMTcyYjU1NzcxMmNiZjI3YTYzM2MzYzZiM2NiNjg2YjI1ZGFjMTIxZTk2ODFjZmQ1NSJ9XX0";
		let decoded_data = general_purpose::STANDARD_NO_PAD
			.decode(encoded_data)
			.unwrap();
		let decoded_str = String::from_utf8(decoded_data).unwrap();

		let folder_options: FolderOptions = serde_json::from_str(&decoded_str).unwrap();

		assert_eq!(
			folder_options.signals,
			vec![
				json!({"type":2, "id":"0xbdb6f0feba303db172b557712cbf27a633c3c6b3cb686b25dac121e9681cfd55"})
			]
		);

		assert_eq!(
			folder_options.signals,
			vec![
				json!({"id":"0xbdb6f0feba303db172b557712cbf27a633c3c6b3cb686b25dac121e9681cfd55", "type":2})
			]
		);
	}
}
