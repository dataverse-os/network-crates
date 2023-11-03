use std::collections::HashMap;

use ceramic_http_client::{FilterQuery, OperationFilter};
use dataverse_types::ceramic::{StreamId, StreamState};

use crate::{index_file::IndexFile, stream::StreamLoader};

#[async_trait::async_trait]
pub trait StreamFileLoader: StreamLoader {
    async fn load_index_file_by_content_id(
        &self,
        ceramic: &String,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)>;
}

#[async_trait::async_trait]
impl StreamFileLoader for dataverse_iroh_store::Client {
    async fn load_index_file_by_content_id(
        &self,
        _ceramic: &String,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)> {
        let streams = self.list_streams_in_model(&model_id).await?;
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

#[async_trait::async_trait]
impl StreamFileLoader for () {
    async fn load_index_file_by_content_id(
        &self,
        ceramic: &String,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)> {
        let ceramic = dataverse_ceramic::http::Client::init(ceramic)?;
        let mut where_filter = HashMap::new();
        where_filter.insert(
            "contentId".to_string(),
            OperationFilter::EqualTo(content_id.clone().into()),
        );
        let filter_query = FilterQuery::Where(where_filter);
        let query_edges = ceramic
            .ceramic
            .query_all(&None, model_id, Some(filter_query))
            .await?;
        if query_edges.len() != 1 {
            anyhow::bail!("index file not found")
        }
        if let Some(edge) = query_edges.first() {
            if let Some(state) = &edge.node {
                return Ok((
                    state.clone(),
                    serde_json::from_value::<IndexFile>(state.content.clone())?,
                ));
            }
        }
        anyhow::bail!("index file not found")
    }
}
