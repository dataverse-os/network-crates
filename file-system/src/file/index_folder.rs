use ceramic_core::Base64String;
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::{Deserialize, Serialize};

use super::{access_control::AccessControl, action_file::ActionType, content_type::ContentType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFolder {
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
struct FolderOptions {
	pub folder_description: Option<String>,
	pub content_type: Option<ContentType>,
	pub action_type: Option<ActionType>,
}

#[repr(u64)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, IntEnum)]
pub enum FolderType {
	PrivateFolderType = 0,
	UnionFolderType = 1,
}
