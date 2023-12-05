extern crate lru;

use ceramic_core::{Cid, StreamId};
use lru::LruCache;
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::Mutex;

use crate::{
    event::{self, Event, EventsUploader, ToCid},
    Ceramic, StreamLoader,
};

use super::{CidLoader, Client};

pub struct Cached {
    pub client: Arc<Client>,
    pub cache: Arc<Mutex<LruCache<Cid, Vec<u8>>>>,
}

impl Cached {
    pub fn new(client: Arc<Client>, cache_size: usize) -> anyhow::Result<Self> {
        let cap = match NonZeroUsize::new(cache_size) {
            Some(cap) => cap,
            None => anyhow::bail!("{} is not a valid cache size", cache_size),
        };
        Ok(Self {
            client,
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
        })
    }
}

impl StreamLoader for Cached {}

#[async_trait::async_trait]
impl CidLoader for Cached {
    async fn load_cid(&self, cid: &Cid) -> anyhow::Result<Vec<u8>> {
        let data_opt;
        {
            let mut cache = self.cache.lock().await;
            data_opt = cache.get(&cid).map(|data| data.to_vec());
        }
        if let Some(data) = data_opt {
            return Ok(data);
        }
        match self.client.load_cid(cid).await {
            Ok(data) => {
                let mut cache = self.cache.lock().await;
                cache.put(cid.clone(), data.to_vec());
                Ok(data)
            }
            Err(err) => Err(err),
        }
    }
}

#[async_trait::async_trait]
impl EventsUploader for Cached {
    async fn upload_event(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()> {
        match &commit.value {
            event::EventValue::Signed(signed) => {
                let mut cache = self.cache.lock().await;

                if let Some(cacao_block) = &signed.cacao_block {
                    cache.put(signed.cacao_link()?, cacao_block.to_vec());
                }
                if let Some(linked_block) = &signed.linked_block {
                    cache.put(signed.payload_link()?, linked_block.to_vec());
                }
                cache.put(commit.cid, signed.jws.to_vec()?);
            }
            // anchor commit generate by ceramic node default
            // don't need to upload it
            event::EventValue::Anchor(_) => {}
        }
        self.client.upload_event(ceramic, stream_id, commit).await
    }
}
