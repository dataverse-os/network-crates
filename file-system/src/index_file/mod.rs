pub mod content_type;
mod index_file;

use ceramic_core::Cid;
pub use index_file::*;

use std::str::FromStr;

use crate::policy::Policy;
use anyhow::Result;
use async_std::task;
use dataverse_ceramic::{self as ceramic, StreamId};
use dataverse_core::store::{dapp::ModelStore, stream::StreamStore};
use serde_json::Value;

use self::content_type::*;

use super::access_control::AccessControl;

struct IndexFileProcessor {
    pub state: ModelState,
    pub model_store: ModelStore,
    pub stream_store: StreamStore,
}

struct ModelState {
    app_id: uuid::Uuid,
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

    async fn validate_content_id(&self, content_id: &str) -> Result<()> {
        if let Ok(stream_id) = StreamId::from_str(content_id) {
            let state = self.stream_store.get_stream(&stream_id).await?;
            let model = self.model_store.get_model(&state.model()?).await?;
            if model.app_id != self.state.app_id {
                anyhow::bail!("stream not in same app");
            }
            // TODO check streamId not fs stream
            // TODO check streamId is Dapp stream
            // TODO check streamId can get from ceramic
        }
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
                log::debug!("content_id is ipfs cid: {}", cid);
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn parse_index_file() {
        let index_file = json!({
          "fileName": "lfcMzQrSOjIdBDupp2Or9Gdp1qrnrcQcCov2t9m34ec",
          "fileType": 2,
          "contentId": "kjzl6kcym7w8y8syiams0kvm3qwfnutk2szi0wlhvf6rr9lalzpibxed0qvotuy",
          "createdAt": "2023-09-01T07:03:23.313Z",
          "fsVersion": "0.11",
          "updatedAt": "2023-09-01T07:55:37.537Z",
          "contentType": "eyJyZXNvdXJjZSI6IkNFUkFNSUMiLCJyZXNvdXJjZUlkIjoia2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1In0",
          "accessControl": "eyJlbmNyeXB0aW9uUHJvdmlkZXIiOnsicHJvdG9jb2wiOiJMaXQiLCJlbmNyeXB0ZWRTeW1tZXRyaWNLZXkiOiI1ODczNjBmMjc3MjUwM2FiZDI0Y2Y2M2RhMjI1MDAwNWNhYjc3ZDlhNjY4NTUyZTdiZDM3MjhlOGE3M2UzMGQ0YzQ2Mjc5NjExZDI5ZDgwN2JmZWVlNThjMGY4ZDFlMGRjNGJhOWI5MWMxMTMwYWUxMWZlZGViZDdlYzdmODkzNGJjZWNkZGQ3MTdlMjRhOTkyNDU1OTY3MjhjNTAxZGI5MjU1YjhiYTFmN2ZhYWIxOWFiOTk2ZjZkZjAzYWI3OTQwZWVmMmVlZGU0ZDMxODIxYTE4NGY5YzVjYmFkMjVlNWViYjE0OTczNjM0NjJlZGUyZmZmNTU1Yjk3MDQ0MzhhMDAwMDAwMDAwMDAwMDAyMGRjNTAzZjExZjdjNmU3MGM0NDMyZWY5ZjdhYjZhM2E4ZDgwNWZhY2YxNjlkMmFlNmYwYjY2MmZhY2VmM2E0YTk1ZDczMGY5OTFlZTBmMjhiZjk5N2ViODcxMDIwMDBiNiIsImRlY3J5cHRpb25Db25kaXRpb25zIjpbeyJjb25kaXRpb25UeXBlIjoiZXZtQmFzaWMiLCJjb250cmFjdEFkZHJlc3MiOiIiLCJzdGFuZGFyZENvbnRyYWN0VHlwZSI6IlNJV0UiLCJjaGFpbiI6ImV0aGVyZXVtIiwibWV0aG9kIjoiIiwicGFyYW1ldGVycyI6WyI6cmVzb3VyY2VzIl0sInJldHVyblZhbHVlVGVzdCI6eyJjb21wYXJhdG9yIjoiY29udGFpbnMiLCJ2YWx1ZSI6ImNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndSJ9fSx7Im9wZXJhdG9yIjoiYW5kIn0seyJjb25kaXRpb25UeXBlIjoiZXZtQmFzaWMiLCJjb250cmFjdEFkZHJlc3MiOiIiLCJzdGFuZGFyZENvbnRyYWN0VHlwZSI6IlNJV0UiLCJjaGFpbiI6ImV0aGVyZXVtIiwibWV0aG9kIjoiIiwicGFyYW1ldGVycyI6WyI6cmVzb3VyY2VzIl0sInJldHVyblZhbHVlVGVzdCI6eyJjb21wYXJhdG9yIjoiY29udGFpbnMiLCJ2YWx1ZSI6ImNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM3Z3U4OGc2NnoyOG44MWxjcGJnNmh1MnQ4cHUycHVpMHNmbnB2c3JocW4za3hoOXhhaSJ9fSx7Im9wZXJhdG9yIjoiYW5kIn0seyJjb25kaXRpb25UeXBlIjoiZXZtQmFzaWMiLCJjb250cmFjdEFkZHJlc3MiOiIiLCJzdGFuZGFyZENvbnRyYWN0VHlwZSI6IlNJV0UiLCJjaGFpbiI6ImV0aGVyZXVtIiwibWV0aG9kIjoiIiwicGFyYW1ldGVycyI6WyI6cmVzb3VyY2VzIl0sInJldHVyblZhbHVlVGVzdCI6eyJjb21wYXJhdG9yIjoiY29udGFpbnMiLCJ2YWx1ZSI6ImNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4Nmd0OWo0MTV5dzJ4OHN0bWtvdGNyenBldXRyYmtwNDJpNHo5MGdwNWlicHR6NHNzbyJ9fSx7Im9wZXJhdG9yIjoiYW5kIn0seyJjb25kaXRpb25UeXBlIjoiZXZtQmFzaWMiLCJjb250cmFjdEFkZHJlc3MiOiIiLCJzdGFuZGFyZENvbnRyYWN0VHlwZSI6IlNJV0UiLCJjaGFpbiI6ImV0aGVyZXVtIiwibWV0aG9kIjoiIiwicGFyYW1ldGVycyI6WyI6cmVzb3VyY2VzIl0sInJldHVyblZhbHVlVGVzdCI6eyJjb21wYXJhdG9yIjoiY29udGFpbnMiLCJ2YWx1ZSI6ImNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhdGVrMzZoM3BlcDA5azlneW1mbmxhOWs2b2psZ3Jtd2pvZ3ZqcWc4cTN6cHlibDF5dSJ9fSx7Im9wZXJhdG9yIjoiYW5kIn0sW3siY29uZGl0aW9uVHlwZSI6ImV2bUJhc2ljIiwiY29udHJhY3RBZGRyZXNzIjoiIiwic3RhbmRhcmRDb250cmFjdFR5cGUiOiIiLCJjaGFpbiI6ImV0aGVyZXVtIiwibWV0aG9kIjoiIiwicGFyYW1ldGVycyI6WyI6dXNlckFkZHJlc3MiXSwicmV0dXJuVmFsdWVUZXN0Ijp7ImNvbXBhcmF0b3IiOiI9IiwidmFsdWUiOiIweDMxMmVBODUyNzI2RTNBOWY2MzNBMDM3N2MwZWE4ODIwODZkNjY2NjYifX0seyJvcGVyYXRvciI6Im9yIn0seyJjb250cmFjdEFkZHJlc3MiOiIweDg2NzNmMjFCMzQzMTlCRDA3MDlBN2E1MDFCRDBmZEI2MTRBMGE3QTEiLCJjb25kaXRpb25UeXBlIjoiZXZtQ29udHJhY3QiLCJmdW5jdGlvbk5hbWUiOiJpc0NvbGxlY3RlZCIsImZ1bmN0aW9uUGFyYW1zIjpbIjp1c2VyQWRkcmVzcyJdLCJmdW5jdGlvbkFiaSI6eyJpbnB1dHMiOlt7ImludGVybmFsVHlwZSI6ImFkZHJlc3MiLCJuYW1lIjoidXNlciIsInR5cGUiOiJhZGRyZXNzIn1dLCJuYW1lIjoiaXNDb2xsZWN0ZWQiLCJvdXRwdXRzIjpbeyJpbnRlcm5hbFR5cGUiOiJib29sIiwibmFtZSI6IiIsInR5cGUiOiJib29sIn1dLCJzdGF0ZU11dGFiaWxpdHkiOiJ2aWV3IiwidHlwZSI6ImZ1bmN0aW9uIn0sImNoYWluIjoibXVtYmFpIiwicmV0dXJuVmFsdWVUZXN0Ijp7ImtleSI6IiIsImNvbXBhcmF0b3IiOiI9IiwidmFsdWUiOiJ0cnVlIn19XV0sImRlY3J5cHRpb25Db25kaXRpb25zVHlwZSI6IlVuaWZpZWRBY2Nlc3NDb250cm9sQ29uZGl0aW9uIn0sIm1vbmV0aXphdGlvblByb3ZpZGVyIjp7InByb3RvY29sIjoiTGVucyIsImJhc2VDb250cmFjdCI6IjB4NzU4MjE3N0Y5RTUzNmFCMGI2YzcyMWUxMWYzODNDMzI2RjJBZDFENSIsInVuaW9uQ29udHJhY3QiOiIweDc1ODIxNzdGOUU1MzZhQjBiNmM3MjFlMTFmMzgzQzMyNkYyQWQxRDUiLCJjaGFpbklkIjo4MDAwMSwiZGF0YXRva2VuSWQiOiIweDg2NzNmMjFCMzQzMTlCRDA3MDlBN2E1MDFCRDBmZEI2MTRBMGE3QTEifX0"
        });
    }
}
