use async_trait::async_trait;
use dataverse_ceramic::{event::EventValue, stream::EventsLoader};
use dataverse_types::ceramic::{self, StreamId, StreamState};
use int_enum::IntEnum;
use json_patch::{Patch, PatchOperation};
use serde_json::Value;

#[async_trait::async_trait]
pub trait Policy: Send + Sync {
    async fn effect_at(&self, _state: &ceramic::StreamState) -> anyhow::Result<bool> {
        Ok(false)
    }
    fn protected_fields(&self) -> Vec<String> {
        vec![]
    }
    async fn validate_data(
        &self,
        _state: &ceramic::StreamState,
        _data: Value,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn validate_patches(&self, _patch: &PatchOperation) -> anyhow::Result<()> {
        Ok(())
    }
    async fn validate_patch_add_or_replace(
        &self,
        _data: &Value,
        _path: &String,
        _value: &serde_json::Value,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
trait PolicyStreamLoader {
    async fn load_stream_with_policies(
        &self,
        stream_id: &StreamId,
        policies: Vec<Box<dyn Policy>>,
    ) -> anyhow::Result<ceramic::StreamState>;
}

#[async_trait]
impl<T: EventsLoader + Sync> PolicyStreamLoader for T {
    async fn load_stream_with_policies(
        &self,
        stream_id: &StreamId,
        policies: Vec<Box<dyn Policy>>,
    ) -> anyhow::Result<ceramic::StreamState> {
        let events = self.load_events(stream_id).await?;

        let mut stream_state: StreamState = StreamState {
            r#type: stream_id.r#type.int_value(),
            ..Default::default()
        };

        for event in events {
            for ele in &policies {
                if ele.effect_at(&stream_state).await? {
                    if let EventValue::Signed(signed) = &event.value {
                        match signed.is_gensis() {
                            true => ele.validate_data(&stream_state, signed.data()?).await?,
                            false => {
                                ele.validate_patch(&stream_state.content, signed.patch()?)
                                    .await?
                            }
                        };
                    }
                }
            }
            event.apply_to(&mut stream_state)?;
        }

        Ok(stream_state)
    }
}

static mut POLICIES: Vec<Box<dyn Policy>> = vec![];

#[async_trait::async_trait]
trait PolicyProcessor {
    fn register_policy(policy: Box<dyn Policy>);

    async fn validate_patch(&self, data: &Value, patches: Patch) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl PolicyProcessor for dyn Policy {
    fn register_policy(policy: Box<dyn Policy>) {
        unsafe {
            POLICIES.push(policy);
        }
    }

    async fn validate_patch(&self, data: &Value, patches: Patch) -> anyhow::Result<()> {
        for patch in patches.0.iter() {
            // check if modify the protected fields
            for ele in patch.path() {
                if self.protected_fields().contains(&ele) {
                    anyhow::bail!("modify protected fields");
                };
            }

            Policy::validate_patches(self, patch).await?;
            let result = match patch {
                PatchOperation::Add(op) => {
                    Policy::validate_patch_add_or_replace(self, &data, &op.path, &op.value).await
                }
                PatchOperation::Replace(op) => {
                    Policy::validate_patch_add_or_replace(self, &data, &op.path, &op.value).await
                }
                _ => Ok(()),
            };
            if result.is_err() {
                anyhow::bail!("validate patch failed");
            }
        }
        Ok(())
    }
}

trait PatchOperationTrait {
    fn path(&self) -> Vec<String>;
    fn value(&self) -> Option<Value>;
}

impl PatchOperationTrait for PatchOperation {
    fn path(&self) -> Vec<String> {
        match self {
            PatchOperation::Add(op) => vec![op.path.clone()],
            PatchOperation::Remove(op) => vec![op.path.clone()],
            PatchOperation::Replace(op) => vec![op.path.clone()],
            PatchOperation::Move(op) => vec![op.path.clone(), op.from.clone()],
            PatchOperation::Copy(op) => vec![op.path.clone(), op.from.clone()],
            PatchOperation::Test(op) => vec![op.path.clone()],
        }
    }

    fn value(&self) -> Option<Value> {
        match self {
            PatchOperation::Add(op) => Some(op.value.clone()),
            PatchOperation::Replace(op) => Some(op.value.clone()),
            PatchOperation::Test(op) => Some(op.value.clone()),
            _ => None,
        }
    }
}
