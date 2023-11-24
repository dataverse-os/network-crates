use dataverse_types::ceramic::StreamState;
use libipld::cid::Cid;
use libipld::{cbor::DagCborCodec, codec::Codec};
use serde::{Deserialize, Serialize};

use super::StreamStateApplyer;

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnchorValue {
    pub id: Cid,
    pub prev: Cid,
    pub proof: Cid,
    pub path: String,

    pub proof_block: Option<Vec<u8>>,
}

impl AnchorValue {
    pub fn proof(&self) -> anyhow::Result<AnchorProof> {
        if let Some(proof_block) = &self.proof_block {
            let node = DagCborCodec.decode(proof_block)?;
            return libipld::serde::from_ipld::<AnchorProof>(node).map_err(|e| e.into());
        }
        anyhow::bail!("no proof block")
    }
}

impl StreamStateApplyer for AnchorValue {
    fn apply_to(&self, stream_state: &mut StreamState) -> anyhow::Result<()> {
        stream_state.anchor_proof = Some(self.proof()?.into());
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnchorProof {
    pub chain_id: String,
    pub root: Cid,
    pub tx_hash: Cid,
    pub tx_type: Option<String>,
}

impl Into<dataverse_types::ceramic::AnchorProof> for AnchorProof {
    fn into(self) -> dataverse_types::ceramic::AnchorProof {
        dataverse_types::ceramic::AnchorProof {
            chain_id: self.chain_id,
            root: self.root.to_bytes().into(),
            tx_hash: self.tx_hash.to_bytes().into(),
            tx_type: self.tx_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use libipld::Ipld;

    use super::*;

    #[test]
    fn decode_anchor() {
        let data = vec![
            164, 100, 114, 111, 111, 116, 216, 42, 88, 37, 0, 1, 113, 18, 32, 207, 168, 82, 146,
            21, 182, 223, 25, 66, 200, 254, 64, 1, 34, 102, 17, 253, 203, 63, 115, 212, 223, 233,
            78, 130, 165, 11, 117, 233, 247, 127, 170, 102, 116, 120, 72, 97, 115, 104, 216, 42,
            88, 38, 0, 1, 147, 1, 27, 32, 141, 27, 16, 141, 128, 187, 139, 165, 10, 133, 142, 28,
            12, 216, 162, 223, 178, 117, 205, 144, 225, 105, 253, 183, 130, 98, 241, 48, 253, 83,
            212, 212, 102, 116, 120, 84, 121, 112, 101, 106, 102, 40, 98, 121, 116, 101, 115, 51,
            50, 41, 103, 99, 104, 97, 105, 110, 73, 100, 104, 101, 105, 112, 49, 53, 53, 58, 49,
        ];
        let node: Ipld = DagCborCodec.decode(&data).unwrap();
        let proof = libipld::serde::from_ipld::<AnchorProof>(node);
        assert!(proof.is_ok());
    }
}
