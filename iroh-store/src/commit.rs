use std::str::FromStr;

use anyhow::{Context, Ok};
use ceramic_core::{Base64String, Jws, StreamId};
use ceramic_core::{Cid, StreamIdType};
use dataverse_ceramic::event::{self, EventValue};
use dataverse_ceramic::jws::ToCid;
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
    pub commit: Content,
    pub opts: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub jws: Jws,
    pub linked_block: Base64String,
    pub cacao_block: Base64String,
}

impl TryInto<event::Event> for Content {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<event::Event, Self::Error> {
        Ok(event::Event {
            cid: self.jws.cid()?,
            value: EventValue::Signed(event::SignedValue {
                jws: self.jws,
                linked_block: Some(self.linked_block.to_vec()?),
                cacao_block: Some(self.cacao_block.to_vec()?),
            }),
        })
    }
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
                    "payload": "AXESIMnfzbG-k1039sJMGOiSotQoXSLkSd7sYRIx6socc21I",
                    "signatures": [
                        {
                            "protected": "eyJhbGciOiJFZERTQSIsImNhcCI6ImlwZnM6Ly9iYWZ5cmVpY3EzczJydmlzbGsycnRxajdqZTd4amlpYmNqN2ZjNmd4bHNtNGhmeGFzN3BnNmV4YzZ5bSIsImtpZCI6ImRpZDprZXk6ejZNa3REVkRVaEVhdUxiRUVaTVNBdFIxNzdkRHljZG96Y3hSZndQcVQyalFWSlU3I3o2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVNyJ9",
                            "signature": "wOjjvtKoyPl93aBmcCITwEiFqBTdGaYm9tkx0xyZPCngrzPeX5TYYWdXV1VLOvcc5aNnYU1fyqc3dRaoLV9SBA"
                        }
                    ],
                    "link": "bafyreigj37g3dputlu37nqsmddujfiwufbosfzcj33wgcerr5lfby43nja"
                },
                "linkedBlock": "o2JpZNgqWCYAAYUBEiBg5f963SDvWCdfJhnxwE7m0CIl1BNANfX9pvpPAPfmCWRkYXRhhKNib3BncmVwbGFjZWRwYXRoai91cGRhdGVkQXRldmFsdWV4GDIwMjMtMTAtMDdUMDg6MjM6MzIuMjI4WqNib3BncmVwbGFjZWRwYXRoaS9maWxlVHlwZWV2YWx1ZQKjYm9wZ3JlcGxhY2VkcGF0aGkvZmlsZU5hbWVldmFsdWV4K0xkNW83VDlJdUtKMW45UFpHTGdvcHoyOW1FTUI4Y3pQdTdWVUFrQ2xMTkGjYm9wY2FkZGRwYXRobi9hY2Nlc3NDb250cm9sZXZhbHVleQ2vZXlKbGJtTnllWEIwYVc5dVVISnZkbWxrWlhJaU9uc2ljSEp2ZEc5amIyd2lPaUpNYVhRaUxDSmxibU55ZVhCMFpXUlRlVzF0WlhSeWFXTkxaWGtpT2lKaE5EbGtNVGM1WXpSbU1UaGpNRFl6T1RKaVkySmlORFk0T0dWaFltVmtNV1ptTW1NME9EVmtNbUU0T0RsalpqUm1NakU1TkRBMU1XVXhZVGhsWVRWaE4yVXpNMkUxWW1OaE9EQXlaamt4TnpSak9HSmxNVGRsWWpjek1tTmpNVFV5TkdKbU1qUXhZak5rWVRrd09EbGpZVEkwTURZek16Y3paVEkyTm1KaU1HTmpOemt3TURNME1EaGxNelJtWm1WaVlqTXhNMlU0TXpFNU4ySTRPVFkzWkRVMU1UTTVaak5oTWpneE5XTXlOalZqT1RjeVl6YzNOakpsWkdVd1lUazBPRGhoTUdNM05HTm1OamMxT0RObVpqTTVNREl5WmpVeFlXSmxNRE5oTkRZMVpqUXpNemt4Tm1JME1qazVNR00zT1dRNE5XTmpZbU00TkRFd05XSmpNREF3TURBd01EQXdNREF3TURBeU1EYzRZV0ZsTW1Zd09EazBZamhsWVRZMlpqSXdPV0UxTVRkaVlURmhaak15T0dZeE5URTFObVU1TUdaaVpHTXlaRGxsTXpoaVptVTNNVGt3Wm1Wall6UTVaamM1TW1Ka1kyTTBObU15TldRMllUWmhOV1V3Wm1RM1lqYzVObVk1T0NJc0ltUmxZM0o1Y0hScGIyNURiMjVrYVhScGIyNXpJanBiZXlKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFFtRnphV01pTENKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJaUxDSnpkR0Z1WkdGeVpFTnZiblJ5WVdOMFZIbHdaU0k2SWxOSlYwVWlMQ0pqYUdGcGJpSTZJbVYwYUdWeVpYVnRJaXdpYldWMGFHOWtJam9pSWl3aWNHRnlZVzFsZEdWeWN5STZXeUk2Y21WemIzVnlZMlZ6SWwwc0luSmxkSFZ5YmxaaGJIVmxWR1Z6ZENJNmV5SmpiMjF3WVhKaGRHOXlJam9pWTI5dWRHRnBibk1pTENKMllXeDFaU0k2SW1ObGNtRnRhV002THk4cVAyMXZaR1ZzUFd0cWVtdzJhSFptY21KM05tTmhaM1EyT1RScGFXMHlkM1ZsWTNVM1pYVnRaV1J6TjNGa01IQTJkWHB0T0dSdWNYTnhOamxzYkRkcllXTnRNRFZuZFNKOWZTeDdJbTl3WlhKaGRHOXlJam9pWVc1a0luMHNleUpqYjI1a2FYUnBiMjVVZVhCbElqb2laWFp0UW1GemFXTWlMQ0pqYjI1MGNtRmpkRUZrWkhKbGMzTWlPaUlpTENKemRHRnVaR0Z5WkVOdmJuUnlZV04wVkhsd1pTSTZJbE5KVjBVaUxDSmphR0ZwYmlJNkltVjBhR1Z5WlhWdElpd2liV1YwYUc5a0lqb2lJaXdpY0dGeVlXMWxkR1Z5Y3lJNld5STZjbVZ6YjNWeVkyVnpJbDBzSW5KbGRIVnlibFpoYkhWbFZHVnpkQ0k2ZXlKamIyMXdZWEpoZEc5eUlqb2lZMjl1ZEdGcGJuTWlMQ0oyWVd4MVpTSTZJbU5sY21GdGFXTTZMeThxUDIxdlpHVnNQV3RxZW13MmFIWm1jbUozTm1NM1ozVTRPR2MyTm5veU9HNDRNV3hqY0dKbk5taDFNblE0Y0hVeWNIVnBNSE5tYm5CMmMzSm9jVzR6YTNob09YaGhhU0o5ZlN4N0ltOXdaWEpoZEc5eUlqb2lZVzVrSW4wc2V5SmpiMjVrYVhScGIyNVVlWEJsSWpvaVpYWnRRbUZ6YVdNaUxDSmpiMjUwY21GamRFRmtaSEpsYzNNaU9pSWlMQ0p6ZEdGdVpHRnlaRU52Ym5SeVlXTjBWSGx3WlNJNklsTkpWMFVpTENKamFHRnBiaUk2SW1WMGFHVnlaWFZ0SWl3aWJXVjBhRzlrSWpvaUlpd2ljR0Z5WVcxbGRHVnljeUk2V3lJNmNtVnpiM1Z5WTJWeklsMHNJbkpsZEhWeWJsWmhiSFZsVkdWemRDSTZleUpqYjIxd1lYSmhkRzl5SWpvaVkyOXVkR0ZwYm5NaUxDSjJZV3gxWlNJNkltTmxjbUZ0YVdNNkx5OHFQMjF2WkdWc1BXdHFlbXcyYUhabWNtSjNObU00Tm1kME9XbzBNVFY1ZHpKNE9ITjBiV3R2ZEdOeWVuQmxkWFJ5WW10d05ESnBOSG81TUdkd05XbGljSFI2TkhOemJ5SjlmU3g3SW05d1pYSmhkRzl5SWpvaVlXNWtJbjBzZXlKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFFtRnphV01pTENKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJaUxDSnpkR0Z1WkdGeVpFTnZiblJ5WVdOMFZIbHdaU0k2SWxOSlYwVWlMQ0pqYUdGcGJpSTZJbVYwYUdWeVpYVnRJaXdpYldWMGFHOWtJam9pSWl3aWNHRnlZVzFsZEdWeWN5STZXeUk2Y21WemIzVnlZMlZ6SWwwc0luSmxkSFZ5YmxaaGJIVmxWR1Z6ZENJNmV5SmpiMjF3WVhKaGRHOXlJam9pWTI5dWRHRnBibk1pTENKMllXeDFaU0k2SW1ObGNtRnRhV002THk4cVAyMXZaR1ZzUFd0cWVtdzJhSFptY21KM05tTmhkR1ZyTXpab00zQmxjREE1YXpsbmVXMW1ibXhoT1dzMmIycHNaM0p0ZDJwdlozWnFjV2M0Y1RONmNIbGliREY1ZFNKOWZTeDdJbTl3WlhKaGRHOXlJam9pWVc1a0luMHNXM3NpWTI5dVpHbDBhVzl1Vkhsd1pTSTZJbVYyYlVKaGMybGpJaXdpWTI5dWRISmhZM1JCWkdSeVpYTnpJam9pSWl3aWMzUmhibVJoY21SRGIyNTBjbUZqZEZSNWNHVWlPaUlpTENKamFHRnBiaUk2SW1WMGFHVnlaWFZ0SWl3aWJXVjBhRzlrSWpvaUlpd2ljR0Z5WVcxbGRHVnljeUk2V3lJNmRYTmxja0ZrWkhKbGMzTWlYU3dpY21WMGRYSnVWbUZzZFdWVVpYTjBJanA3SW1OdmJYQmhjbUYwYjNJaU9pSTlJaXdpZG1Gc2RXVWlPaUl3ZURVNU1UVmxNamt6T0RJelJrTmhPRFF3WXprelJVUXlSVEZGTlVJMFpHWXpNbVEyT1RrNU9Ua2lmWDBzZXlKdmNHVnlZWFJ2Y2lJNkltOXlJbjBzZXlKamIyNTBjbUZqZEVGa1pISmxjM01pT2lJd2VFVkdPREUzTXpObU9USkRObU14TkVOaFl6azJNRGt4TnpoalJqaGhZemcwWWtaalpUSXlNallpTENKamIyNWthWFJwYjI1VWVYQmxJam9pWlhadFEyOXVkSEpoWTNRaUxDSm1kVzVqZEdsdmJrNWhiV1VpT2lKcGMwTnZiR3hsWTNSbFpDSXNJbVoxYm1OMGFXOXVVR0Z5WVcxeklqcGJJanAxYzJWeVFXUmtjbVZ6Y3lKZExDSm1kVzVqZEdsdmJrRmlhU0k2ZXlKcGJuQjFkSE1pT2x0N0ltbHVkR1Z5Ym1Gc1ZIbHdaU0k2SW1Ga1pISmxjM01pTENKdVlXMWxJam9pZFhObGNpSXNJblI1Y0dVaU9pSmhaR1J5WlhOekluMWRMQ0p1WVcxbElqb2lhWE5EYjJ4c1pXTjBaV1FpTENKdmRYUndkWFJ6SWpwYmV5SnBiblJsY201aGJGUjVjR1VpT2lKaWIyOXNJaXdpYm1GdFpTSTZJaUlzSW5SNWNHVWlPaUppYjI5c0luMWRMQ0p6ZEdGMFpVMTFkR0ZpYVd4cGRIa2lPaUoyYVdWM0lpd2lkSGx3WlNJNkltWjFibU4wYVc5dUluMHNJbU5vWVdsdUlqb2liWFZ0WW1GcElpd2ljbVYwZFhKdVZtRnNkV1ZVWlhOMElqcDdJbXRsZVNJNklpSXNJbU52YlhCaGNtRjBiM0lpT2lJOUlpd2lkbUZzZFdVaU9pSjBjblZsSW4xOVhWMHNJbVJsWTNKNWNIUnBiMjVEYjI1a2FYUnBiMjV6Vkhsd1pTSTZJbFZ1YVdacFpXUkJZMk5sYzNORGIyNTBjbTlzUTI5dVpHbDBhVzl1SW4wc0ltMXZibVYwYVhwaGRHbHZibEJ5YjNacFpHVnlJanA3SW5CeWIzUnZZMjlzSWpvaVRHVnVjeUlzSW1KaGMyVkRiMjUwY21GamRDSTZJakI0TnpVNE1qRTNOMFk1UlRVek5tRkNNR0kyWXpjeU1XVXhNV1l6T0RORE16STJSakpCWkRGRU5TSXNJblZ1YVc5dVEyOXVkSEpoWTNRaU9pSXdlRGMxT0RJeE56ZEdPVVUxTXpaaFFqQmlObU0zTWpGbE1URm1Nemd6UXpNeU5rWXlRV1F4UkRVaUxDSmphR0ZwYmtsa0lqbzRNREF3TVN3aVpHRjBZWFJ2YTJWdVNXUWlPaUl3ZUVWR09ERTNNek5tT1RKRE5tTXhORU5oWXprMk1Ea3hOemhqUmpoaFl6ZzBZa1pqWlRJeU1qWWlmWDBkcHJldtgqWCYAAYUBEiBg5f963SDvWCdfJhnxwE7m0CIl1BNANfX9pvpPAPfmCQ",
                "cacaoBlock": "o2FooWF0Z2VpcDQzNjFhcKljYXVkeDhkaWQ6a2V5Ono2TWt0RFZEVWhFYXVMYkVFWk1TQXRSMTc3ZER5Y2RvemN4UmZ3UHFUMmpRVkpVN2NleHB4GDIwMjMtMTAtMTRUMDc6Mjk6MjMuMTAyWmNpYXR4GDIwMjMtMTAtMDdUMDc6Mjk6MjMuMTAyWmNpc3N4O2RpZDpwa2g6ZWlwMTU1OjE6MHg1OTE1ZTI5MzgyM0ZDYTg0MGM5M0VEMkUxRTVCNGRmMzJkNjk5OTk5ZW5vbmNlbkRkbjdsU2MzdlFUd3F2ZmRvbWFpbnggY2VrcGZua2xjaWZpb21nZW9nYm1rbm5tY2dia2RwaW1ndmVyc2lvbmExaXJlc291cmNlc4p4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4c29nY2M0MzhmZ2dzdW55YnVxNnE5ZWN4b2FvemN4ZThxbGprOHd1M3VxdTM5NHV4N3hRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2F0ZWszNmgzcGVwMDlrOWd5bWZubGE5azZvamxncm13am9ndmpxZzhxM3pweWJsMXl1eFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN3hsdGh6eDlkaXk2azNyM3MweGFmOGg3NG5neGhuY2dqd3llcGw1OHBrYTE1eDl5aGN4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmM4NjFjenZkc2xlZDN5bHNhOTk3N2k3cmxvd3ljOWw3anBnNmUxaGp3aDlmZWZsNmJzdXhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Y2I0bXNkODhpOG1sanp5cDNhencwOXgyNnYza2pvamVpdGJleDE4MWVmaTk0ZzU4ZWxmeFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjN2d1ODhnNjZ6MjhuODFsY3BiZzZodTJ0OHB1MnB1aTBzZm5wdnNyaHFuM2t4aDl4YWl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhd3JsN2Y3NjdiNmN6NDhkbjBlZnI5d2Z0eDl0OWplbHc5dGIxb3R4ejc1MmpoODZrbnhRY2VyYW1pYzovLyo/bW9kZWw9a2p6bDZodmZyYnc2Yzg2Z3Q5ajQxNXl3Mng4c3Rta290Y3J6cGV1dHJia3A0Mmk0ejkwZ3A1aWJwdHo0c3NveFFjZXJhbWljOi8vKj9tb2RlbD1ranpsNmh2ZnJidzZjNnZiNjR3aTg4dWI0N2dibWNoODJ3Y3BibWU1MWh5bTRzOXFicDJ1a2FjMHl0aHpiajl4UWNlcmFtaWM6Ly8qP21vZGVsPWtqemw2aHZmcmJ3NmNhZ3Q2OTRpaW0yd3VlY3U3ZXVtZWRzN3FkMHA2dXptOGRucXNxNjlsbDdrYWNtMDVndWlzdGF0ZW1lbnR4MUdpdmUgdGhpcyBhcHBsaWNhdGlvbiBhY2Nlc3MgdG8gc29tZSBvZiB5b3VyIGRhdGFhc6Jhc3iEMHhmZDI0ZmVkNTA0MmFlMjdjYmY1NmUxN2FmNmJmZjdhNDQwZTZkMTY1NGZiNzhmZWQ4ZDNiYjdiN2RjOTRhMmFjMmY1MmU3M2EwMDdlZDhlMDExNzA2MGYyNzZjNTk2MTNhOGQ2OWI4NjgyNTJlYjZiMWE0MWE3ZGFkZWFlMzY3MzFiYXRmZWlwMTkx"
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
            "bagcqcerai6gutyaooolz437gwh3zvdty2dvosvnoib7po5gox5xoyuyq3bda"
        );

        Ok(())
    }
}
