use ceramic_core::StreamId;
use dataverse_ceramic::StreamState;
use dataverse_file_system::file::{IndexFile, StreamFileLoader};

use crate::Client;

#[async_trait::async_trait]
impl StreamFileLoader for Client {
    async fn load_index_file_by_content_id(
        &self,
        _ceramic: &String,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)> {
        let streams = self.list_stream_states_in_model(&model_id).await?;
        for ele in streams {
            if let Ok(index_file) = serde_json::from_value::<IndexFile>(ele.content.clone()) {
                if index_file.content_id == *content_id {
                    return Ok((ele, index_file));
                }
            }
        }
        anyhow::bail!("index file not found")
    }
}
