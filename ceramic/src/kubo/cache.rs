extern crate lru;

use ceramic_core::Cid;
use fang::{AsyncQueue, AsyncQueueable, NoTls};
use lru::LruCache;
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::Mutex;

use crate::StreamLoader;

use super::{
	message::MessagePublisher,
	task::{BlockUploadHandler, UpdateMessagePublishHandler},
	BlockUploader, CidLoader, Client,
};

pub struct Cached {
	pub client: Arc<Client>,
	pub queue: Arc<Mutex<AsyncQueue<NoTls>>>,
	pub cache: Arc<Mutex<LruCache<Cid, Vec<u8>>>>,
}

impl Cached {
	pub fn new(
		client: Arc<Client>,
		queue: Arc<Mutex<AsyncQueue<NoTls>>>,
		cache_size: usize,
	) -> anyhow::Result<Self> {
		let cap = match NonZeroUsize::new(cache_size) {
			Some(cap) => cap,
			None => anyhow::bail!("{} is not a valid cache size", cache_size),
		};
		Ok(Self {
			client,
			queue,
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
impl BlockUploader for Cached {
	async fn block_upload(&self, cid: Cid, block: Vec<u8>) -> anyhow::Result<()> {
		self.cache.lock().await.put(cid, block.clone());
		let task = BlockUploadHandler { cid, block };
		if let Err(err) = self.queue.lock().await.insert_task(&task).await {
			log::error!("failed to insert task: {}", err);
		};
		Ok(())
	}
}

#[async_trait::async_trait]
impl MessagePublisher for Cached {
	async fn publish_message(&self, topic: &String, msg: Vec<u8>) -> anyhow::Result<()> {
		let task = UpdateMessagePublishHandler {
			topic: topic.clone(),
			msg,
		};
		if let Err(err) = self.queue.lock().await.insert_task(&task).await {
			log::error!("failed to insert task: {}", err);
		};
		Ok(())
	}
}
