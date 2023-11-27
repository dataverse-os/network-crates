use super::Stream;

#[async_trait::async_trait]
pub trait StreamPublisher {
    async fn publish_all_streams(&self) -> anyhow::Result<()>;
    async fn publish_stream(&self, stream: Stream) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl StreamPublisher for () {
    async fn publish_all_streams(&self) -> anyhow::Result<()> {
        todo!("publish streams");
    }

    async fn publish_stream(&self, _stream: Stream) -> anyhow::Result<()> {
        todo!("publish stream");
    }
}
