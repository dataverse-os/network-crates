use ceramic_core::Base64String;
use chrono::{DateTime, Utc};
use int_enum::IntEnum;
use serde::Deserialize;

use crate::{
    access_control::AccessControl, action_file::action_file::ActionType,
    index_file::content_type::ContentType,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFolder {
    pub options: Base64String,

    pub created_at: DateTime<Utc>,
    pub fs_version: String,
    pub updated_at: DateTime<Utc>,

    pub folder_name: String,
    pub folder_type: FolderType,
    pub access_control: Base64String,
    pub content_folder_ids: Vec<String>,
}

impl IndexFolder {
    pub fn options(&self) -> anyhow::Result<FolderOptions> {
        Ok(serde_json::from_slice(&self.options.to_vec()?)?)
    }
    pub fn access_control(&self) -> anyhow::Result<AccessControl> {
        Ok(serde_json::from_slice(&self.access_control.to_vec()?)?)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderOptions {
    pub folder_description: Option<String>,
    pub content_type: Option<ContentType>,
    pub action_type: Option<ActionType>,
}

#[repr(u64)]
#[derive(Debug, Deserialize, Clone, Copy, IntEnum)]
pub enum FolderType {
    PrivateFolderType = 0,
    UnionFolderType = 1,
}
