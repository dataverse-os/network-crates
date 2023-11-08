// https://github.com/ChainAgnostic/CAIPs/blob/main/CAIPs/caip-74.md

use std::collections::HashMap;

use ceramic_core::StreamId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct CACAO {
    pub h: Header,    // container meta-information
    pub p: Payload,   // payload
    pub s: Signature, // signature, single
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Header {
    pub t: String, // specifies format of the payload
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    pub domain: String, // =domain
    pub iss: String,    // = DID pkh
    pub aud: String,    // =uri
    pub version: String,
    pub nonce: String,
    pub iat: String,                    // RFC3339 date-time = issued-at
    pub nbf: Option<String>,            // RFC3339 date-time = not-before
    pub exp: Option<String>,            // RFC3339 date-time = expiration-time
    pub statement: Option<String>,      // =statement
    pub request_id: Option<String>,     // =request-id
    pub resources: Option<Vec<String>>, // =resources as URIs
}

impl Payload {
    pub fn issued_at(&self) -> anyhow::Result<DateTime<Utc>> {
        Ok(self.iat.parse::<DateTime<Utc>>()?)
    }
    pub fn not_before(&self) -> anyhow::Result<Option<DateTime<Utc>>> {
        if let Some(nbf) = &self.nbf {
            return Ok(Some(nbf.parse::<DateTime<Utc>>()?));
        }
        Ok(None)
    }
    pub fn expiration_time(&self) -> anyhow::Result<Option<DateTime<Utc>>> {
        if let Some(exp) = &self.exp {
            return Ok(Some(exp.parse::<DateTime<Utc>>()?));
        }
        Ok(None)
    }

    pub fn resource_models(&self) -> anyhow::Result<Vec<StreamId>> {
        let mut result: Vec<StreamId> = Vec::new();
        if let Some(resources) = &self.resources {
            for resource in resources {
                let url = url::Url::parse(resource)?;
                if url.scheme() == "ceramic" {
                    let hash_query: HashMap<_, _> = url.query_pairs().into_owned().collect();
                    if let Some(model_id) = hash_query.get("model") {
                        let model_id = model_id.parse::<StreamId>()?;
                        result.push(model_id);
                    };
                }
            }
        }
        Ok(result)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Signature {
    pub t: String,
    pub m: Option<SignatureMeta>,
    pub s: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignatureMeta {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cacao() {
        let cacao = serde_json::json!({
          "h": {
            "t": "eip4361"
          },
          "p": {
            "aud": "http://localhost:3000/login",
            "exp": "2022-03-10T18:09:21.481+03:00",
            "iat": "2022-03-10T17:09:21.481+03:00",
            "iss": "did:pkh:eip155:1:0xBAc675C310721717Cd4A37F6cbeA1F081b1C2a07",
            "nbf": "2022-03-10T17:09:21.481+03:00",
            "nonce": "328917",
            "domain": "localhost:3000",
            "version": "1",
            "requestId": "request-id-random",
            "resources": [
              "ipfs://bafybeiemxf5abjwjbikoz4mc3a3dla6ual3jsgpdr4cjr3oz3evfyavhwq",
              "https://example.com/my-web2-claim.json"
            ],
            "statement": "I accept the ServiceOrg Terms of Service: https://service.org/tos"
          },
          "s": {
            "s": "5ccb134ad3d874cbb40a32b399549cd32c953dc5dc87dc64624a3e3dc0684d7d4833043dd7e9f4a6894853f8dc555f97bc7e3c7dd3fcc66409eb982bff3a44671b",
            "t": "eip191"
          }
        });

        let cacao = serde_json::from_value::<CACAO>(cacao);
        assert!(cacao.is_ok());
        let cacao = cacao.unwrap();
        let iat = cacao.p.issued_at();
        println!("{:?}", iat);
        assert!(iat.is_ok());
        assert_eq!(
            iat.unwrap(),
            "2022-03-10T14:09:21.481Z".parse::<DateTime<Utc>>().unwrap()
        )
    }
}
