use std::collections::HashMap;

use ceramic_http_client::{FilterQuery, OperationFilter};
use dataverse_ceramic::{StreamId, StreamState};
use dataverse_core::stream::StreamOperator;

use super::index_file::IndexFile;

#[async_trait::async_trait]
pub trait StreamFileLoader: StreamOperator {
    async fn load_index_file_by_content_id(
        &self,
        ceramic: &String,
        model_id: &StreamId,
        content_id: &String,
    ) -> anyhow::Result<(StreamState, IndexFile)>;
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
            .query_all(None, model_id, Some(filter_query))
            .await?;
        if query_edges.len() != 1 {
            anyhow::bail!("index file not found")
        }
        if let Some(edge) = query_edges.first() {
            if let Some(state) = &edge.node {
                return Ok((
                    state.clone().try_into()?,
                    serde_json::from_value::<IndexFile>(state.content.clone())?,
                ));
            }
        }
        anyhow::bail!("index file not found")
    }
}
