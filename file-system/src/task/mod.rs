use fang::{AsyncQueue, AsyncQueueable, AsyncRunnable, NoTls};

pub struct TaskQueue {
    queue: AsyncQueue<NoTls>,
}

impl TaskQueue {
    pub async fn new(dsn: &str, max_pool_size: u32) -> anyhow::Result<Self> {
        let mut queue = AsyncQueue::builder()
            .uri(dsn)
            // Max number of connections that are allowed
            .max_pool_size(max_pool_size)
            .build();

        // Always connect first in order to perform any operation
        queue.connect(NoTls).await?;

        Ok(Self { queue })
    }

    pub async fn insert_task(&mut self, task: &dyn AsyncRunnable) -> anyhow::Result<()> {
        self.queue.insert_task(task).await?;
        Ok(())
    }
}
