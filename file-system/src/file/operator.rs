use std::collections::HashMap;

use ceramic_http_client::{FilterQuery, OperationFilter};
use dataverse_ceramic::{event::EventsUploader, Ceramic, StreamId, StreamState, StreamsLoader};

use super::index_file::IndexFile;

#[async_trait::async_trait]
pub trait StreamFileLoader: StreamsLoader + EventsUploader {
    async fn load_index_file_by_content_id(
        &self,
        ceramic: &Ceramic,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)> {
        let streams = self.load_stream_states(ceramic, None, model_id).await?;
        for ele in streams {
            if let Ok(index_file) = serde_json::from_value::<IndexFile>(ele.content.clone()) {
                if index_file.content_id == *content_id {
                    return Ok((ele, index_file));
                }
            }
        }
        anyhow::bail!("index file with content_id {} not found", content_id)
    }
}

#[async_trait::async_trait]
impl StreamFileLoader for dataverse_ceramic::http::Client {
    async fn load_index_file_by_content_id(
        &self,
        ceramic: &Ceramic,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)> {
        let mut where_filter = HashMap::new();
        where_filter.insert(
            "contentId".to_string(),
            OperationFilter::EqualTo(content_id.clone().into()),
        );

        let query = Some(FilterQuery::Where(where_filter));
        let streams = self.query_model(ceramic, None, model_id, query).await?;
        if streams.len() != 1 {
            anyhow::bail!("index file not found")
        }

        let state = match streams.first() {
            Some(state) => state,
            _ => anyhow::bail!("index file with contentId {} not found", content_id),
        };
        Ok((
            state.clone(),
            serde_json::from_value::<IndexFile>(state.content.clone())?,
        ))
    }
}
