extern crate lru;

use ceramic_core::Cid;
use lru::LruCache;
use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex},
};

use super::{CidLoader, Client};

pub struct Cached {
    pub client: Client,
    pub cache: Arc<Mutex<LruCache<String, String>>>,
}

impl Cached {
    pub fn new(client: Client) -> anyhow::Result<Self> {
        let env = std::env::var("CERAMIC_CACHE_SIZE").unwrap_or("1000".to_string());
        let n = env.parse()?;
        let cap = match NonZeroUsize::new(n) {
            Some(cap) => cap,
            None => anyhow::bail!("CERAMIC_CACHE_SIZE {} is not a valid cache size", n),
        };
        Ok(Self {
            client,
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
        })
    }
}

#[async_trait::async_trait]
impl CidLoader for Cached {
    async fn load_cid(&self, cid: &Cid) -> anyhow::Result<Vec<u8>> {
        let cid_str = cid.to_string();
        let data = {
            let mut cache = self.cache.lock().unwrap();
            cache.get(&cid_str).map(|data| data.as_bytes().to_vec())
        };
        if let Some(data) = data {
            return Ok(data);
        }
        match self.client.load_cid(cid).await {
            Ok(data) => {
                let mut cache = self.cache.lock().unwrap();
                cache.put(cid_str, String::from_utf8(data.clone())?);
                Ok(data)
            }
            Err(err) => Err(err),
        }
    }
}
