use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use dataverse_ceramic::{StreamId, StreamState};
use dataverse_core::store::dapp::ModelStore;

use crate::index_file::IndexFile;
use crate::stream::StreamOperator;

use super::FileModel;
use super::{loader::StreamFileLoader, StreamFile};

trait StreamFileOperator: StreamFileLoader + StreamOperator + Send + Sync {}

pub struct Client<'a> {
    model_store: &'a ModelStore,
    loader: Arc<dyn StreamFileLoader + Send + Sync>,
}

impl Client<'_> {
    pub fn new(loader: Option<Arc<dataverse_iroh_store::Client>>) -> Self {
        match loader {
            Some(iroh) => Self {
                model_store: ModelStore::get_instance(),
                loader: iroh,
            },
            None => Self {
                model_store: ModelStore::get_instance(),
                loader: Arc::new(()),
            },
        }
    }
}

impl Client<'_> {
    pub async fn get_file_model(
        &self,
        app_id: &uuid::Uuid,
        model: FileModel,
    ) -> anyhow::Result<dataverse_core::store::dapp::Model> {
        self.model_store
            .get_model_by_name(&app_id, &model.to_string())
            .await
    }

    pub async fn get_dapp_ceramic(&self, app_id: &uuid::Uuid) -> anyhow::Result<String> {
        Ok(self
            .model_store
            .get_model_by_name(&app_id, &FileModel::IndexFile.to_string())
            .await?
            .indexed_on)
    }

    pub async fn load_stream_by_app_id(
        &self,
        app_id: &uuid::Uuid,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let ceramic = self.get_dapp_ceramic(app_id).await?;
        self.loader.load_stream(&ceramic, stream_id).await
    }

    pub async fn load_streams_auto_model(
        &self,
        account: &Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let model = self.model_store.get_model(model_id).await?;
        self.loader
            .load_streams(account, &model.indexed_on, model_id)
            .await
    }
}

#[async_trait::async_trait]
pub trait StreamFileTrait {
    async fn load_file(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamFile>;

    async fn load_stream(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamState>;

    async fn load_files(
        &self,
        account: &Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamFile>>;
}

#[async_trait::async_trait]
impl StreamFileTrait for Client<'_> {
    async fn load_file(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamFile> {
        let ceramic = self.get_dapp_ceramic(dapp_id).await?;
        let stream_state = self.loader.load_stream(&ceramic, &stream_id).await?;
        let model = self.model_store.get_model(&stream_state.model()?).await?;
        match model.model_name.as_str() {
            "indexFile" => {
                let index_file = serde_json::from_value::<IndexFile>(stream_state.content.clone())?;
                let mut file = StreamFile::new_with_file(stream_state)?;
                if let Ok(content_id) = &index_file.content_id.parse() {
                    let content_state = self.loader.load_stream(&ceramic, &content_id).await?;
                    file.write_content(content_state)?;
                }
                Ok(file)
            }
            "actionFile" => StreamFile::new_with_file(stream_state),
            "indexFolder" | "contentFolder" => StreamFile::new_with_content(stream_state),
            _ => {
                let mut file = StreamFile::new_with_content(stream_state)?;
                let model_id = self
                    .get_file_model(&dapp_id, FileModel::IndexFile)
                    .await?
                    .model_id;

                let index_file = self
                    .loader
                    .load_index_file_by_content_id(&ceramic, &model_id, &stream_id.to_string())
                    .await;

                match index_file {
                    Ok((state, _)) => {
                        file.write_content(state)?;
                    }
                    _ => {
                        file.verified_status = -1;
                    }
                }
                Ok(file)
            }
        }
    }

    async fn load_stream(
        &self,
        app_id: &uuid::Uuid,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let model_index_file = self.get_file_model(&app_id, FileModel::IndexFile).await?;
        self.loader
            .load_stream(&model_index_file.indexed_on, stream_id)
            .await
    }

    async fn load_files(
        &self,
        account: &Option<String>,
        model_id: &StreamId,
    ) -> Result<Vec<StreamFile>> {
        let model = self.model_store.get_model(&model_id).await?;
        let app_id = model.app_id;
        let ceramic = model.indexed_on;

        let stream_states = self
            .loader
            .load_streams(account, &ceramic, &model_id)
            .await?;

        match model.model_name.as_str() {
            "indexFile" => {
                let mut files: Vec<StreamFile> = vec![];
                for state in stream_states {
                    let index_file: IndexFile = serde_json::from_value(state.content.clone())?;
                    let mut file = StreamFile::new_with_file(state)?;
                    file.content_id = Some(index_file.content_id.clone());

                    if let Ok(stream_id) = &index_file.content_id.parse() {
                        let content_state = self.loader.load_stream(&ceramic, stream_id).await?;
                        file.write_content(content_state)?;
                    }
                    files.push(file);
                }

                Ok(files)
            }
            "actionFile" => stream_states
                .into_iter()
                .map(StreamFile::new_with_file)
                .collect(),
            "indexFolder" | "contentFolder" => stream_states
                .into_iter()
                .map(StreamFile::new_with_content)
                .collect(),
            _ => {
                let model_index_file = self.get_file_model(&app_id, FileModel::IndexFile).await?;

                let file_query_edges = self
                    .loader
                    .load_streams(
                        &account,
                        &model_index_file.indexed_on,
                        &model_index_file.model_id,
                    )
                    .await?;

                let mut file_map: HashMap<String, StreamFile> = HashMap::new();
                for state in stream_states {
                    let content_id = state.stream_id()?;
                    let file = StreamFile::new_with_content(state)?;
                    file_map.insert(content_id.to_string(), file);
                }

                for node in file_query_edges {
                    let index_file = serde_json::from_value::<IndexFile>(node.content.clone());
                    if let Ok(index_file) = index_file {
                        if let Some(stream_file) = file_map.get_mut(&index_file.content_id) {
                            stream_file.file_model_id = Some(model_index_file.model_id.clone());
                            stream_file.file_id = Some(node.stream_id()?);
                            stream_file.file = Some(node.content);
                        }
                    }
                }

                // set verified_status to -1 if file_id is None (illegal file)
                let files = file_map
                    .into_iter()
                    .map(|(_, mut file)| {
                        if file.file_id.is_none() {
                            file.verified_status = -1;
                        }
                        file
                    })
                    .collect();

                Ok(files)
            }
        }
    }
}
