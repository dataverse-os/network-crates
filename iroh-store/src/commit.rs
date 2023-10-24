use std::str::FromStr;

use anyhow::Context;
use ceramic_core::{Base64String, Jws, StreamId};
use ceramic_core::{Cid, StreamIdType};
use dataverse_types::ceramic::event;
use dataverse_types::ceramic::jws::ToCid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Genesis {
    pub r#type: u64,
    pub genesis: Content,
    pub opts: serde_json::Value,
}

impl Genesis {
    pub fn model_id(&self) -> anyhow::Result<StreamId> {
        let payload = self.genesis.payload()?;
        payload.header.map(|x| x.model).context("missing model id")
    }

    pub fn stream_id(&self) -> anyhow::Result<StreamId> {
        let stream_id = StreamId {
            r#type: StreamIdType::try_from(self.r#type)?,
            cid: self.genesis.cid()?,
        };
        Ok(stream_id)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub stream_id: StreamId,
    pub commit: DataCommit,
    pub opts: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataCommit {
    pub jws: Content,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub jws: Jws,
    pub linked_block: Base64String,
    pub cacao_block: Base64String,
}

impl Content {
    pub fn payload(&self) -> anyhow::Result<event::Payload> {
        event::Payload::try_from(self.linked_block.to_vec()?)
    }

    pub fn cid(&self) -> anyhow::Result<Cid> {
        Ok(Cid::from_str(&self.jws.cid()?.to_string())?)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{from_value, json};

    use super::*;

    #[test]
    fn test_decode_gensis_commit() -> anyhow::Result<()> {
        let commit = from_value::<Genesis>(json!({
            "type": 3,
            "genesis": {
                "jws": {
                    "payload": "AXESIHlwcfYaDjgakHz5vbzICzt9KABN0ZGfK-yofbOigqmw",
                    "signatures": [
                      {
                        "signature": "mhhY_--rw6pOWStvPa-lQ4iIYLPeabx7lE9fG5MC5A_nYdoyJEXIObCnJjlNYUZPPjTw2RcZlov_idBN6csnBw",
                        "protected": "eyJhbGciOiJFZERTQSIsImNhcCI6ImlwZnM6Ly9iYWZ5cmVpZndzNmxtanVkc3Z2dWZyNHM0dnpmdWxhb2tuNzRjN3RpajRxMzR4eGhocmVvdTVwYXJvYSIsImtpZCI6ImRpZDprZXk6ejZNa2dXMTUzcWRidTUxQnZ2dFlOWnpDVUxHRDJza0tpM2sxSHR5S3ZOQWRCcnFTI3o2TWtnVzE1M3FkYnU1MUJ2dnRZTlp6Q1VMR0Qyc2tLaTNrMUh0eUt2TkFkQnJxUyJ9"
                      }
                    ],
                    "link": "bafyreidzoby7mgqohanja7hzxw6mqcz3puuaatorsgpsx3fipwz2favjwa"
                  },
                "linkedBlock": "omRkYXRhp2hmaWxlTmFtZWRwb3N0aGZpbGVUeXBlAGljb250ZW50SWR4P2tqemw2a2N5bTd3OHk3eG54dnBqd3NwbGUzbHl3b3pxb2k1dGtyeGF2a2V3N2NvYmpoMDk3ZDNrZDI5cGd4NmljcmVhdGVkQXR4GDIwMjMtMDktMDZUMDU6MjI6NTAuMzM4Wmlmc1ZlcnNpb25kMC4xMWl1cGRhdGVkQXR4GDIwMjMtMDktMDZUMDU6MjI6NTAuMzM4Wmtjb250ZW50VHlwZXiHZXlKeVpYTnZkWEpqWlNJNklrTkZVa0ZOU1VNaUxDSnlaWE52ZFhKalpVbGtJam9pYTJwNmJEWm9kbVp5WW5jMlkyRjBaV3N6Tm1nemNHVndNRGxyT1dkNWJXWnViR0U1YXpadmFteG5jbTEzYW05bmRtcHhaemh4TTNwd2VXSnNNWGwxSW4wZmhlYWRlcqRjc2VwZW1vZGVsZW1vZGVsWCjOAQIBhQESIH8JG4Y2KIV/LJ/ZtDn5+K80Ln63tgcVD+fDPvKyFFHIZnVuaXF1ZUx23XKCIao/IA/UZSJrY29udHJvbGxlcnOBeDtkaWQ6cGtoOmVpcDE1NToxOjB4MzEyZUE4NTI3MjZFM0E5ZjYzM0EwMzc3YzBlYTg4MjA4NmQ2NjY2Ng",
                "cacaoBlock": "o2FooWF0Z2VpcDQzNjFhcKljYXVkeDhkaWQ6a2V5Ono2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVN2NleHB4GDIwMjMtMTAtMTRUMDc6Mjk6MjMuMTAyWmNpYXR4GDIwMjMtMTAtMDdUMDc6Mjk6MjMuMTAyWmNpc3N4O2RpZDpwa2g6ZWlwMTU1OjE6MHg1OTE1ZTI5MzgyM0ZDYTg0MGM5M0VEMkUxRTVCNGRmMzJkNjk5OTk5ZW5vbmNlbkRkbjdsU2MzdlFUd3F2ZmRvbWFpbnggY2VrcGZua2xjaWZpb21nZW9nYm1rbm5tY2dia2RwaW1ndmVyc2lvbmExaXJlc291cmNlc4p4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4c29nY2M0MzhmZ2dzdW55YnVxNnE5ZWN4b2FvemN4ZThxbGprOHd1M3VxdTM5NHV4N3hRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1eFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN3hsdGh6eDlkaXk2azNyM3MweGFmOGg3NG5neGhuY2dqd3llcGw1OHBrYTE1eDl5aGN4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4NjFjenZkc2xlZDN5bHNhOTk3N2k3cmxvd3ljOWw3anBnNmUxaGp3aDlmZWZsNmJzdXhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2I0bXNkODhpOG1sanp5cDNhencwOXgyNnYza2pvamVpdGJleDE4MWVmaTk0ZzU4ZWxmeFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN2d1ODhnNjZ6MjhuODFsY3BiZzZodTJ0OHB1MnB1aTBzZm5wdnNyaHFuM2t4aDl4YWl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhd3JsN2Y3NjdiNmN6NDhkbjBlZnI5d2Z0eDl0OWplbHc5dGIxb3R4ejc1MmpoODZrbnhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Yzg2Z3Q5ajQxNXl3Mng4c3Rta290Y3J6cGV1dHJia3A0Mmk0ejkwZ3A1aWJwdHo0c3NveFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjNnZiNjR3aTg4dWI0N2dibWNoODJ3Y3BibWU1MWh5bTRzOXFicDJ1a2FjMHl0aHpiajl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndWlzdGF0ZW1lbnR4MUdpdmUgdGhpcyBhcHBsaWNhdGlvbiBhY2Nlc3MgdG8gc29tZSBvZiB5b3VyIGRhdGFhc6Jhc3iEMHhmZDI0ZmVkNTA0MmFlMjdjYmY1NmUxN2FmNmJmZjdhNDQwZTZkMTY1NGZiNzhmZWQ4ZDNiYjdiN2RjOTRhMmFjMmY1MmU3M2EwMDdlZDhlMDExNzA2MGYyNzZjNTk2MTNhOGQ2OWI4NjgyNTJlYjZiMWE0MWE3ZGFkZWFlMzY3MzFiYXRmZWlwMTkx"
            },
            "opts": {
                "anchor": true,
                "publish": true,
                "sync": 3,
                "syncTimeoutSeconds": 0
            }
        }));

        assert!(commit.is_ok());
        let commit = commit.unwrap();

        let payload = commit.genesis.payload();
        assert!(payload.is_ok());
        let payload = payload.unwrap();
        assert_eq!(
            payload.header.unwrap().model.to_string(),
            "kjzl6hvfrbw6c86gt9j415yw2x8stmkotcrzpeutrbkp42i4z90gp5ibptz4sso"
        );

        let cid = commit.genesis.cid()?;
        println!("commit cid: {}", cid);
        assert_eq!(
            cid.to_string(),
            "bagcqceraeeto3737ppwcmowjns25bilelzipyxrb4ehjmxz2a3dzbk4llfaq"
        );

        let stream_id = commit.stream_id()?;

        println!("stream_id: {}", stream_id);
        assert_eq!(
            stream_id.to_string(),
            "kjzl6kcym7w8y5pj1xs5iotnbplg7x4hgoohzusuvk8s7oih3h2fuplcvwvu2wx"
        );

        Ok(())
    }

    #[test]
    fn test_decode_data_commit() -> anyhow::Result<()> {
        let commit = from_value::<Data>(json!({
            "streamId": "kjzl6kcym7w8y7aq5fcqraw3vk69f2syk6kpcmcs6xojujxf9batubj5ibki495",
            "commit": {
                "jws": {
                    "jws": {
                      "payload": "AXESIHxzfskfPqnVaNNckO-QBM9sqbpzK31LM2ZFCQcAJYaN",
                      "signatures": [
                        {
                          "signature": "m_3D1aCQZ1xQsJJl-uR8tdoZgvdQKiY_pasG6xqjopt4Fuk82ku5TGGY1rgy7RZY5edjG-7_O4YGBgdxQUDpCA",
                          "protected": "eyJhbGciOiJFZERTQSIsImNhcCI6ImlwZnM6Ly9iYWZ5cmVpZmhvYm91bms1aG0yNjZpbjc3ZngyNmozbzVsM3Vza2hzeTZ3ZWFoaXl4bXpoZW9qNGxydSIsImtpZCI6ImRpZDprZXk6ejZNa29WNktqRnFMeGlleHoxRm9lTWtYbTJzS0hwMjRaRVEzZ3B6azdLNjhMQktFI3o2TWtvVjZLakZxTHhpZXh6MUZvZU1rWG0yc0tIcDI0WkVRM2dwems3SzY4TEJLRSJ9"
                        }
                      ],
                      "link": "bafyreid4on7mshz6vhkwru24sdxzabgpnsu3u4zlpvftgzsfbedqajmgru"
                    },
                    "linkedBlock": "o2JpZNgqWCYAAYUBEiAhJu3/f3vsJjrJbLXQoWReUPxeIeEOll86BseQq4tZQWRkYXRhgGRwcmV22CpYJgABhQESICEm7f9/e+wmOslstdChZF5Q/F4h4Q6WXzoGx5Cri1lB",
                    "cacaoBlock": "o2FooWF0Z2VpcDQzNjFhcKljYXVkeDhkaWQ6a2V5Ono2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVN2NleHB4GDIwMjMtMTAtMTRUMDc6Mjk6MjMuMTAyWmNpYXR4GDIwMjMtMTAtMDdUMDc6Mjk6MjMuMTAyWmNpc3N4O2RpZDpwa2g6ZWlwMTU1OjE6MHg1OTE1ZTI5MzgyM0ZDYTg0MGM5M0VEMkUxRTVCNGRmMzJkNjk5OTk5ZW5vbmNlbkRkbjdsU2MzdlFUd3F2ZmRvbWFpbnggY2VrcGZua2xjaWZpb21nZW9nYm1rbm5tY2dia2RwaW1ndmVyc2lvbmExaXJlc291cmNlc4p4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4c29nY2M0MzhmZ2dzdW55YnVxNnE5ZWN4b2FvemN4ZThxbGprOHd1M3VxdTM5NHV4N3hRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1eFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN3hsdGh6eDlkaXk2azNyM3MweGFmOGg3NG5neGhuY2dqd3llcGw1OHBrYTE1eDl5aGN4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4NjFjenZkc2xlZDN5bHNhOTk3N2k3cmxvd3ljOWw3anBnNmUxaGp3aDlmZWZsNmJzdXhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2I0bXNkODhpOG1sanp5cDNhencwOXgyNnYza2pvamVpdGJleDE4MWVmaTk0ZzU4ZWxmeFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN2d1ODhnNjZ6MjhuODFsY3BiZzZodTJ0OHB1MnB1aTBzZm5wdnNyaHFuM2t4aDl4YWl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhd3JsN2Y3NjdiNmN6NDhkbjBlZnI5d2Z0eDl0OWplbHc5dGIxb3R4ejc1MmpoODZrbnhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Yzg2Z3Q5ajQxNXl3Mng4c3Rta290Y3J6cGV1dHJia3A0Mmk0ejkwZ3A1aWJwdHo0c3NveFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjNnZiNjR3aTg4dWI0N2dibWNoODJ3Y3BibWU1MWh5bTRzOXFicDJ1a2FjMHl0aHpiajl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndWlzdGF0ZW1lbnR4MUdpdmUgdGhpcyBhcHBsaWNhdGlvbiBhY2Nlc3MgdG8gc29tZSBvZiB5b3VyIGRhdGFhc6Jhc3iEMHhmZDI0ZmVkNTA0MmFlMjdjYmY1NmUxN2FmNmJmZjdhNDQwZTZkMTY1NGZiNzhmZWQ4ZDNiYjdiN2RjOTRhMmFjMmY1MmU3M2EwMDdlZDhlMDExNzA2MGYyNzZjNTk2MTNhOGQ2OWI4NjgyNTJlYjZiMWE0MWE3ZGFkZWFlMzY3MzFiYXRmZWlwMTkx"
                },
            },
            "opts": {
                "anchor": true,
                "publish": true,
                "sync": 3
            }
        }));

        assert!(commit.is_ok());
        let commit = commit.unwrap();

        let cid = commit.commit.jws.cid()?;
        println!("commit cid: {}", cid);
        assert_eq!(
            cid.to_string(),
            "bagcqcerad4ksqqygh5wux6ephrnbyppy3ij2tpwxqf2dlsa4mefhkptlvtpa"
        );

        Ok(())
    }
}
