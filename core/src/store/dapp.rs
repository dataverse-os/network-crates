use std::collections::HashMap;

use anyhow::Context;
use ceramic_core::StreamId;
use dataverse_ceramic::{
    network::{Chain, Network},
    Ceramic,
};
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct Model {
    pub model_id: StreamId,
    pub app_id: uuid::Uuid,
    pub encryptable: Vec<String>,
    pub model_name: String,
    pub version: i32,
    pub indexed_on: String,
    pub chains: Vec<Chain>,
}

impl Model {
    pub fn ceramic(&self) -> anyhow::Result<Ceramic> {
        Ok(Ceramic {
            endpoint: self.clone().indexed_on,
            network: self.network()?,
        })
    }
    pub fn network(&self) -> anyhow::Result<Network> {
        let chain = self.chains.first().context("ceramic not in networks")?;
        Ok(chain.network())
    }
}

static MODEL_STORE: Lazy<ModelStore> = Lazy::new(ModelStore::new);

pub struct ModelStore {
    client: dapp_table_client::Client,
    models: HashMap<String, Model>,
}

impl ModelStore {
    fn new() -> Self {
        let backend = std::env::var("DAPP_TABLE_BACKEND").ok();
        ModelStore {
            models: HashMap::new(),
            client: dapp_table_client::Client::new(backend),
        }
    }

    pub fn get_instance() -> &'static ModelStore {
        &MODEL_STORE
    }

    pub async fn get_dapp_ceramic(&self, dapp_id: &uuid::Uuid) -> anyhow::Result<String> {
        let dapp = self
            .client
            .lookup_dapp_by_dapp_id(&dapp_id.to_string())
            .await?;
        Ok(dapp.ceramic)
    }

    pub async fn get_models(&self, dapp_id: &uuid::Uuid) -> anyhow::Result<Vec<Model>> {
        let dapp = self
            .client
            .lookup_dapp_by_dapp_id(&dapp_id.to_string())
            .await?;
        let chains = dataverse_ceramic::http::Client::chains(&dapp.ceramic).await?;

        let mut models = vec![];
        for model in dapp.models {
            let stream = model.streams.last().expect("get length 0 of model streams");

            models.push(Model {
                model_id: stream.model_id.parse()?,
                app_id: dapp.id.parse()?,
                encryptable: stream.encryptable.clone(),
                model_name: model.model_name,
                version: model.streams.len() as i32,
                indexed_on: dapp.ceramic.clone(),
                chains: chains.clone(),
            });
        }
        Ok(models)
    }

    pub async fn get_model_by_name(
        &self,
        dapp_id: &uuid::Uuid,
        model_name: &str,
    ) -> anyhow::Result<Model> {
        for ele in &self.models {
            if ele.1.model_name == model_name && ele.1.app_id == *dapp_id {
                return Ok(ele.1.clone());
            }
        }
        let dapp = self
            .client
            .lookup_dapp_by_dapp_id(&dapp_id.to_string())
            .await?;
        let chains = dataverse_ceramic::http::Client::chains(&dapp.ceramic).await?;

        for model in dapp.models {
            if model.model_name == model_name {
                let stream = model.streams.last().expect("get length 0 of model streams");
                return Ok(Model {
                    model_id: stream.model_id.parse()?,
                    app_id: dapp.id.parse()?,
                    encryptable: stream.encryptable.clone(),
                    model_name: model.model_name,
                    version: model.streams.len() as i32,
                    indexed_on: dapp.ceramic.clone(),
                    chains: chains.clone(),
                });
            }
        }

        anyhow::bail!(
            "model with name `{}` not found in dapp {}",
            model_name,
            dapp_id
        )
    }

    pub async fn get_model(&self, model_id: &StreamId) -> anyhow::Result<Model> {
        for ele in &self.models {
            if ele.1.model_id == *model_id {
                return Ok(ele.1.clone());
            }
        }
        match self.lookup_dapp_model_in_db(&model_id).await {
            Ok(model) => Ok(model),
            Err(_) => self.lookup_dapp_model_by_query(&model_id).await,
        }
    }

    pub async fn store_model(&mut self, model: Model) -> anyhow::Result<()> {
        self.models.insert(model.model_id.to_string(), model);
        Ok(())
    }

    pub async fn lookup_dapp_model_in_db(&self, model_id: &StreamId) -> anyhow::Result<Model> {
        match self.models.get_key_value(&model_id.to_string()) {
            Some(kv) => Ok(kv.1.clone()),
            None => anyhow::bail!("model not found"),
        }
    }

    pub async fn lookup_dapp_model_by_query(&self, model_id: &StreamId) -> anyhow::Result<Model> {
        let variables = dapp_table_client::get_dapp::Variables {
            dapp_id: None,
            model_id: Some(model_id.to_string()),
        };
        let dapp = self.client.lookup_dapp(variables).await?;
        let chains = dataverse_ceramic::http::Client::chains(&dapp.ceramic).await?;

        for model in dapp.models {
            for (idx, ele) in model.streams.iter().enumerate() {
                if ele.model_id == model_id.to_string() {
                    return Ok(Model {
                        model_id: ele.model_id.parse()?,
                        app_id: dapp.id.parse()?,
                        encryptable: ele.encryptable.clone(),
                        model_name: model.model_name,
                        version: idx as i32,
                        indexed_on: dapp.ceramic,
                        chains: chains.clone(),
                    });
                }
            }
        }
        anyhow::bail!("model with id `{}` not found in dapp table", model_id)
    }
}
