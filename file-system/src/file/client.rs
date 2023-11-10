use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use dataverse_types::ceramic::{StreamId, StreamState};
use dataverse_types::store::dapp::ModelStore;

use crate::index_file::IndexFile;
use crate::stream::StreamOperator;

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
    pub async fn get_index_file_model(
        &self,
        app_id: &uuid::Uuid,
    ) -> anyhow::Result<dataverse_types::store::dapp::Model> {
        self.model_store
            .get_model_by_name(&app_id, "indexFile")
            .await
    }

    pub async fn get_dapp_ceramic(&self, app_id: &uuid::Uuid) -> anyhow::Result<String> {
        Ok(self
            .model_store
            .get_model_by_name(&app_id, "indexFile")
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

    async fn load_files_by_index_file_model_id(
        &self,
        account: &Option<String>,
        app_id: &uuid::Uuid,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamFile>>;

    async fn load_files_by_model_id(
        &self,
        account: &Option<String>,
        app_id: &uuid::Uuid,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamFile>>;
}

#[async_trait::async_trait]
impl StreamFileTrait for Client<'_> {
    async fn load_file(&self, app_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamFile> {
        let model_index_file = self.get_index_file_model(&app_id).await?;
        let file_model_id = model_index_file.model_id;
        let ceramic = &model_index_file.indexed_on;
        let stream_state = self.loader.load_stream(ceramic, &stream_id).await?;
        let controller = stream_state
            .controllers()
            .first()
            .context("no controler")?
            .clone();

        let file = if stream_state.model()? == file_model_id {
            let index_file = serde_json::from_value::<IndexFile>(stream_state.content)?;
            let content_id = &index_file.content_id.parse();
            match content_id {
                Ok(content_id) => {
                    let stream_state = self.loader.load_stream(ceramic, &content_id).await?;
                    StreamFile {
                        file_id: Some(stream_id.clone()),
                        file_model_id: Some(file_model_id),
                        file: Some(index_file),
                        content_id: content_id.to_string(),
                        model_id: Some(stream_state.model()?),
                        content: Some(stream_state.content),
                        ..Default::default()
                    }
                }
                _ => StreamFile {
                    file_id: Some(stream_id.clone()),
                    file_model_id: Some(file_model_id),
                    content_id: index_file.content_id.clone(),
                    file: Some(index_file),
                    ..Default::default()
                },
            }
        } else {
            let index_file = self
                .loader
                .load_index_file_by_content_id(ceramic, &file_model_id, &stream_id.to_string())
                .await;
            match index_file {
                Ok((state, index_file)) => StreamFile {
                    file_id: Some(state.stream_id()?),
                    file_model_id: Some(file_model_id),
                    file: Some(index_file),
                    content_id: stream_id.to_string(),
                    model_id: Some(stream_state.model()?),
                    content: Some(stream_state.content),
                    ..Default::default()
                },
                _ => StreamFile {
                    content_id: stream_id.to_string(),
                    model_id: Some(stream_state.model()?),
                    content: Some(stream_state.content),
                    ..Default::default()
                },
            }
        };

        Ok(StreamFile { controller, ..file })
    }

    async fn load_stream(
        &self,
        app_id: &uuid::Uuid,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let model_index_file = self.get_index_file_model(&app_id).await?;
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

        match model.model_name.as_str() {
            "indexFile" => {
                self.load_files_by_index_file_model_id(account, &model.app_id, model_id)
                    .await
            }
            _ => {
                self.load_files_by_model_id(account, &model.app_id, model_id)
                    .await
            }
        }
    }

    async fn load_files_by_index_file_model_id(
        &self,
        account: &Option<String>,
        app_id: &uuid::Uuid,
        model_id: &StreamId,
    ) -> Result<Vec<StreamFile>> {
        let model = self.get_index_file_model(&app_id).await?;
        let file_stream_states = self
            .loader
            .load_streams(account, &model.indexed_on, &model_id)
            .await?;

        let mut files: Vec<StreamFile> = vec![];
        for node in file_stream_states {
            let index_file: IndexFile = serde_json::from_value(node.content.clone())?;
            let mut file = StreamFile {
                file_id: Some(node.stream_id()?),
                file_model_id: Some(model.model_id.clone()),
                file: Some(index_file.clone()),
                content_id: index_file.content_id.clone(),
                controller: node.controllers().first().context("no controller")?.clone(),
                ..Default::default()
            };

            if let Ok(content_id) = &index_file.content_id.parse() {
                let stream_state = self
                    .loader
                    .load_stream(&model.indexed_on, content_id)
                    .await?;
                file.model_id = Some(stream_state.model()?);
                file.content = Some(stream_state.content);
            }
            files.push(file);
        }

        Ok(files)
    }

    async fn load_files_by_model_id(
        &self,
        account: &Option<String>,
        app_id: &uuid::Uuid,
        model_id: &StreamId,
    ) -> Result<Vec<StreamFile>> {
        let model_index_file = self.get_index_file_model(&app_id).await?;

        let content_query_edges = self
            .loader
            .load_streams(&account, &model_index_file.indexed_on, &model_id)
            .await?;
        let file_query_edges = self
            .loader
            .load_streams(
                &account,
                &model_index_file.indexed_on,
                &model_index_file.model_id,
            )
            .await?;

        let mut file_map: HashMap<String, StreamFile> = HashMap::new();
        for node in content_query_edges {
            let content_id = node.stream_id()?;
            file_map.insert(
                content_id.to_string(),
                StreamFile {
                    content_id: content_id.to_string(),
                    model_id: Some(model_id.clone()),
                    content: Some(node.content.clone()),
                    controller: node.controllers().first().context("no controller")?.clone(),
                    ..Default::default()
                },
            );
        }

        for node in file_query_edges {
            let index_file = serde_json::from_value::<IndexFile>(node.content.clone());
            if let Ok(index_file) = index_file {
                if let Some(stream_file) = file_map.get_mut(&index_file.content_id) {
                    stream_file.file_model_id = Some(model_index_file.model_id.clone());
                    stream_file.file_id = Some(node.stream_id()?);
                    stream_file.file = serde_json::from_value(node.content.clone())?;
                }
            }
        }

        Ok(file_map.into_iter().map(|(_, v)| v).collect())
    }
}
