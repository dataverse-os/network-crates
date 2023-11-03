pub mod content_type;
mod index_file;

use ceramic_core::Cid;
pub use index_file::*;

use std::str::FromStr;

use crate::policy::Policy;
use anyhow::Result;
use async_std::task;
use dataverse_types::ceramic::{self, StreamId};
use dataverse_types::store::{dapp::ModelStore, stream::StreamStore};
use serde_json::Value;

use self::content_type::*;

use super::access_control::AccessControl;

struct IndexFileProcessor {
    pub state: ModelState,
    pub model_store: ModelStore,
    pub stream_store: StreamStore,
}

#[async_trait::async_trait]
impl Policy for IndexFileProcessor {
    async fn effect_at(&self, state: &ceramic::StreamState) -> Result<bool> {
        // check model_name is indexfile
        let model_id = state.model()?;
        let model = self.model_store.get_model(&model_id).await?;
        Ok(model.model_name == "indexFile")
    }

    async fn validate_data(
        &self,
        _state: &ceramic::StreamState,
        data: serde_json::Value,
    ) -> Result<()> {
        let content: IndexFile = serde_json::from_value(data)?;
        let content_type = content.content_type()?;

        // validate content id
        self.validate_content(&content.content_id, &content_type)
            .await?;
        // check acl
        if let Some(acl) = content.access_control()? {
            self.validate_acl(&acl).await?;
        };
        Ok(())
    }

    async fn validate_patch_add_or_replace(
        &self,
        data: &Value,
        path: &String,
        value: &Value,
    ) -> Result<()> {
        match path.as_str() {
            "/accessControl" => {
                let data = value.as_str().unwrap();
                let acl: AccessControl = AccessControl::from_str(data)?;
                task::block_on(self.validate_acl(&acl))
            }
            "/fileType" => IndexFileProcessor::validate_file_type_modify_constraint(data, value),
            _ => Ok(()),
        }
    }

    fn protected_fields(&self) -> Vec<String> {
        vec!["contentId".to_string(), "contentType".to_string()]
    }
}

impl IndexFileProcessor {
    fn validate_file_type_modify_constraint(data: &Value, _value: &Value) -> Result<()> {
        let index_file: IndexFile = serde_json::from_value(data.clone())?;
        if index_file.file_type == IndexFileType::Payable as u64 {
            anyhow::bail!("file type cannot be changed");
        }
        Ok(())
    }

    fn validate_stream_id(stream_id: &str) -> Result<()> {
        let stream_id = StreamId::from_str(stream_id)?;
        // TODO check streamId not fs stream
        // TODO check streamId is Dapp stream
        // TODO check streamId can get from ceramic
        Ok(())
    }

    async fn validate_content(
        &self,
        content_id: &String,
        content_type: &ContentType,
    ) -> Result<()> {
        match content_type.resource {
            ContentTypeResourceType::IPFS => {
                let cid = Cid::from_str(&content_id)?;
                match &cid.codec() {
                    0x70 => {
                        // TODO check cid in lighthouse
                    }
                    _ => {
                        // TODO check cid in global ipfs
                    }
                }
            }
            ContentTypeResourceType::CERAMIC => {
                if let Some(resource_id) = &content_type.resource_id {
                    let model_id: StreamId = resource_id.parse()?;
                    let content_id: StreamId = content_id.parse()?;
                    let content = self.stream_store.get_stream(&content_id).await?;
                    if model_id != content.model()? {
                        anyhow::bail!("resourceId not match contentId")
                    }
                }
            }
            _ => {}
        };
        Ok(())
    }

    async fn validate_acl(&self, acl: &AccessControl) -> Result<()> {
        if let Some(p) = &acl.encryption_provider {
            let linked_ceramic_models = p.linked_ceramic_models()?;
            for ele in linked_ceramic_models {
                let model = self.model_store.get_model(&ele).await?;
                if model.app_id != self.state.app_id {
                    anyhow::bail!("linked model not in same app");
                }
            }
        }

        Ok(())
    }
}

struct ModelState {
    app_id: uuid::Uuid,
}
