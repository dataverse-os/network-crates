use fang::{AsyncQueue, AsyncWorkerPool, NoTls};

pub type Queue = AsyncQueue<NoTls>;

pub async fn new_queue(dsn: &str, max_pool_size: u32) -> anyhow::Result<Queue> {
	let mut queue = AsyncQueue::builder()
	.uri(dsn)
	// Max number of connections that are allowed
	.max_pool_size(max_pool_size)
	.build();

	// Always connect first in order to perform any operation
	queue.connect(NoTls).await?;
	return Ok(queue);
}

pub fn build_pool(queue: Queue, num: u32) -> AsyncWorkerPool<AsyncQueue<NoTls>> {
	AsyncWorkerPool::builder()
		.number_of_workers(num)
		.queue(queue)
		.build()
}
