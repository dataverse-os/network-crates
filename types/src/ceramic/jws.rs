use std::str::FromStr;

use dag_jose::{DagJoseCodec, JsonWebSignature};
use libipld::{
    multihash::{Code, MultihashDigest},
    prelude::Codec,
    Cid,
};

pub trait ToCid {
    fn cid(&self) -> anyhow::Result<Cid>;
}

impl ToCid for ceramic_core::Jws {
    fn cid(&self) -> anyhow::Result<Cid> {
        let jws: JsonWebSignature = TryIntoJwsSignature::try_into(self)?;
        jws.cid()
    }
}

impl ToCid for JsonWebSignature {
    fn cid(&self) -> anyhow::Result<Cid> {
        Ok(Cid::new_v1(
            0x85,
            Code::Sha2_256.digest(DagJoseCodec.encode(&self)?.as_ref()),
        ))
    }
}

pub trait TryIntoJwsSignature {
    fn try_into(&self) -> anyhow::Result<JsonWebSignature>;
}

impl TryIntoJwsSignature for ceramic_core::Jws {
    fn try_into(&self) -> anyhow::Result<JsonWebSignature> {
        let link = match self.link.clone() {
            Some(val) => val,
            None => anyhow::bail!("JWS does not have a link"),
        };
        let signatures = self
            .signatures
            .iter()
            .map(|x| dag_jose::Signature {
                header: Default::default(),
                protected: x.protected.as_ref().map(|s| s.to_string()),
                signature: x.signature.to_string(),
            })
            .collect();

        Ok(JsonWebSignature {
            payload: self.payload.to_string(),
            signatures,
            link: Cid::from_str(link.as_ref())?,
        })
    }
}
