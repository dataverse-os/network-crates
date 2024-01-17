use fang::{AsyncQueue, AsyncWorkerPool};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;

pub type Queue = AsyncQueue<MakeTlsConnector>;

pub async fn new_queue(dsn: &str, max_pool_size: u32) -> anyhow::Result<Queue> {
	let mut queue = AsyncQueue::builder()
	.uri(dsn)
	// Max number of connections that are allowed
	.max_pool_size(max_pool_size)
	.build();

	let connector = TlsConnector::builder().build()?;
	let tls = MakeTlsConnector::new(connector);
	queue.connect(tls).await?;
	return Ok(queue);
}

pub fn build_pool(queue: Queue, num: u32) -> AsyncWorkerPool<AsyncQueue<MakeTlsConnector>> {
	AsyncWorkerPool::builder()
		.number_of_workers(num)
		.queue(queue)
		.build()
}
