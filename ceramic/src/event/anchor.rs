use dataverse_types::ceramic::StreamState;
use libipld::cid::Cid;
use serde::{Deserialize, Serialize};

use super::StreamStateApplyer;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnchorValue {
    pub id: Cid,
    pub prev: Cid,
    pub proof: Cid,
    pub path: String,
}

impl StreamStateApplyer for AnchorValue {
    fn apply_to(&self, _stream_state: &mut StreamState) -> anyhow::Result<()> {
        todo!("apply anchor value to stream state")
    }
}
