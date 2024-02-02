use ceramic_core::Base64String;
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};

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
	pub signal: Option<serde_json::Value>,
	pub folder_description: Option<String>,
}

#[repr(u64)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, IntEnum)]
pub enum FolderType {
	PrivateFolderType = 0,
	UnionFolderType = 1,
}
