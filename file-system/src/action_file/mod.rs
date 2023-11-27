use ceramic_core::StreamId;
use dataverse_core::store::dapp::ModelStore;

use crate::policy::Policy;

pub mod action_file;

struct ActionFileProcessor {
    pub model_store: ModelStore,
}

#[async_trait::async_trait]
impl Policy for ActionFileProcessor {
    async fn effect_at(
        &self,
        state: &dataverse_ceramic::stream::StreamState,
    ) -> anyhow::Result<bool> {
        // check model_name is indexfile
        let model_id = state.model()?;
        let model = self.model_store.get_model(&model_id).await?;
        Ok(model.model_name == "indexFile")
    }
}

impl ActionFileProcessor {
    // check resource id is type index_file or union_folder
    async fn check_resource_id(&self, realoation_id: StreamId) -> anyhow::Result<()> {
        Ok(())
    }
}
