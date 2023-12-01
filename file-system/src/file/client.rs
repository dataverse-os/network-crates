use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use chrono::Utc;
use dataverse_ceramic::event::{Event, EventValue, VerifyOption};
use dataverse_ceramic::{Ceramic, StreamId, StreamOperator, StreamState};
use dataverse_core::store::dapp::ModelStore;
use dataverse_core::stream::{Stream, StreamStore};
use int_enum::IntEnum;

use super::index_file::IndexFile;
use super::FileModel;
use super::{operator::StreamFileLoader, StreamFile};

trait StreamFileOperator: StreamFileLoader + StreamOperator + Send + Sync {}

pub struct Client<'a> {
    pub model_store: &'a ModelStore,
    pub operator: Arc<dyn StreamFileLoader + Send + Sync>,
    pub stream_store: Arc<dyn StreamStore + Send + Sync>,
}

impl Client<'_> {
    pub fn new(
        operator: Arc<dyn StreamFileLoader + Send + Sync>,
        stream_store: Arc<dyn StreamStore + Send + Sync>,
    ) -> Self {
        Self {
            model_store: ModelStore::get_instance(),
            operator,
            stream_store,
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

    pub async fn get_dapp_ceramic(&self, app_id: &uuid::Uuid) -> anyhow::Result<Ceramic> {
        let model = self
            .model_store
            .get_model_by_name(&app_id, &FileModel::IndexFile.to_string())
            .await?;
        Ok(Ceramic {
            endpoint: model.clone().indexed_on,
            network: model.network()?,
        })
    }

    pub async fn load_stream_by_app_id(
        &self,
        app_id: &uuid::Uuid,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let ceramic = self.get_dapp_ceramic(app_id).await?;

        self.operator
            .load_stream_state(&ceramic, stream_id, None)
            .await
    }

    pub async fn load_streams_auto_model(
        &self,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let model = self.model_store.get_model(model_id).await?;
        self.operator
            .load_stream_states(&model.ceramic()?, account, model_id)
            .await
    }
}

#[async_trait::async_trait]
pub trait StreamFileTrait {
    async fn load_file(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamFile>;

    async fn load_stream(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamState>;

    async fn load_files(
        &self,
        account: Option<String>,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamFile>>;
}

#[async_trait::async_trait]
impl StreamFileTrait for Client<'_> {
    async fn load_file(&self, dapp_id: &uuid::Uuid, stream_id: &StreamId) -> Result<StreamFile> {
        let ceramic = self.get_dapp_ceramic(dapp_id).await?;
        let stream_state = self
            .operator
            .load_stream_state(&ceramic, &stream_id, None)
            .await?;
        let model = self.model_store.get_model(&stream_state.model()?).await?;
        match model.model_name.as_str() {
            "indexFile" => {
                let index_file = serde_json::from_value::<IndexFile>(stream_state.content.clone())?;
                let mut file = StreamFile::new_with_file(stream_state)?;
                if let Ok(content_id) = &index_file.content_id.parse() {
                    let content_state = self
                        .operator
                        .load_stream_state(&ceramic, &content_id, None)
                        .await?;
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
                    .operator
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
        let ceramic = &model_index_file.ceramic()?;
        self.operator
            .load_stream_state(ceramic, stream_id, None)
            .await
    }

    async fn load_files(
        &self,
        account: Option<String>,
        model_id: &StreamId,
    ) -> Result<Vec<StreamFile>> {
        let model = self.model_store.get_model(&model_id).await?;
        let app_id = model.app_id;
        let ceramic = model.ceramic()?;

        let stream_states = self
            .operator
            .load_stream_states(&ceramic, account.clone(), &model_id)
            .await?;

        match model.model_name.as_str() {
            "indexFile" => {
                let mut files: Vec<StreamFile> = vec![];
                for state in stream_states {
                    let index_file: IndexFile = serde_json::from_value(state.content.clone())?;
                    let mut file = StreamFile::new_with_file(state)?;
                    file.content_id = Some(index_file.content_id.clone());

                    if let Ok(stream_id) = &index_file.content_id.parse() {
                        let content_state = self
                            .operator
                            .load_stream_state(&ceramic, stream_id, None)
                            .await?;
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
                    .operator
                    .load_stream_states(&ceramic, account, &model_index_file.model_id)
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

#[async_trait::async_trait]
pub trait StreamEventSaver {
    async fn save_event(
        &self,
        dapp_id: &uuid::Uuid,
        stream_id: &StreamId,
        event: &Event,
    ) -> Result<StreamState>;
}

#[async_trait::async_trait]
impl StreamEventSaver for Client<'_> {
    async fn save_event(
        &self,
        dapp_id: &uuid::Uuid,
        stream_id: &StreamId,
        event: &Event,
    ) -> Result<StreamState> {
        let ceramic = self.model_store.get_dapp_ceramic(dapp_id).await?;
        match &event.value {
            EventValue::Signed(signed) => {
                let (mut stream, mut commits) = {
                    let stream = self.stream_store.load_stream(&stream_id).await;
                    match stream.ok().flatten() {
                        Some(stream) => (
                            stream.clone(),
                            self.operator
                                .load_events(&ceramic, stream_id, Some(stream.tip))
                                .await?,
                        ),
                        None => {
                            if !signed.is_gensis() {
                                anyhow::bail!(
                                    "publishing commit with stream_id {} not found in store",
                                    stream_id
                                );
                            }
                            (
                                Stream::new(dapp_id, stream_id.r#type.int_value(), event, None)?,
                                vec![],
                            )
                        }
                    }
                };
                // check if commit already exists
                if commits.iter().any(|ele| ele.cid == event.cid) {
                    return stream.state(commits);
                }

                if let Some(prev) = event.prev()? {
                    if commits.iter().all(|ele| ele.cid != prev) {
                        anyhow::bail!("donot have prev commit");
                    }
                }
                commits.push(event.clone());
                let state = stream.state(commits)?;

                let model = state.model()?;
                let opts = vec![
                    VerifyOption::ResourceModelsContain(model.clone()),
                    VerifyOption::ExpirationTimeBefore(Utc::now() - chrono::Duration::days(100)),
                ];
                event.verify_signature(opts)?;

                stream.model = Some(model);
                stream.tip = event.cid;

                self.stream_store.save_stream(&stream).await?;
                self.operator
                    .upload_event(&ceramic, &stream_id, event.clone())
                    .await?;

                Ok(state)
            }
            EventValue::Anchor(_) => {
                anyhow::bail!("anchor commit not supported");
            }
        }
    }
}
