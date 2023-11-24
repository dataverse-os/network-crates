mod client;
mod loader;

use std::fmt::Display;

use anyhow::Context;
pub use client::*;
use dataverse_types::ceramic::StreamState;
pub use loader::*;

use ceramic_core::StreamId;
use serde::{Deserialize, Serialize};
use serde_json::value::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_model_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<StreamId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,

    pub controller: String,
    pub verified_status: i64,
}

impl Default for StreamFile {
    fn default() -> Self {
        Self {
            file_id: None,
            file_model_id: None,
            file: Default::default(),
            content_id: Default::default(),
            model_id: Default::default(),
            content: Default::default(),
            verified_status: Default::default(),
            controller: Default::default(),
        }
    }
}

impl StreamFile {
    pub fn new_with_file(state: StreamState) -> anyhow::Result<Self> {
        let mut file = Self::default();
        file.write_file(state)?;
        Ok(file)
    }

    pub fn write_file(&mut self, state: StreamState) -> anyhow::Result<()> {
        self.file = Some(state.content.clone());
        self.file_id = Some(state.stream_id()?);
        self.file_model_id = Some(state.model()?);
        self.controller = state
            .controllers()
            .first()
            .context("no controller")?
            .clone();
        Ok(())
    }

    pub fn new_with_content(state: StreamState) -> anyhow::Result<Self> {
        let mut file = Self::default();
        file.write_content(state)?;
        Ok(file)
    }

    pub fn write_content(&mut self, state: StreamState) -> anyhow::Result<()> {
        self.content = Some(state.content.clone());
        self.content_id = Some(state.stream_id()?.to_string());
        self.model_id = Some(state.model()?);
        self.controller = state
            .controllers()
            .first()
            .context("no controller")?
            .clone();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileModel {
    IndexFile,
    ActionFile,
    IndexFolder,
    ContentFolder,
}

impl Display for FileModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let serialized = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", serialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_file() -> anyhow::Result<()> {
        println!("{}", FileModel::IndexFile);
        Ok(())
    }
}
