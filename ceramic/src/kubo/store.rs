use ceramic_core::{Cid, StreamId};

#[async_trait::async_trait]
pub trait Store: Sync + Send {
    async fn add(&self, id: String, stream_id: &StreamId) -> anyhow::Result<()>;
    async fn exists(&self, id: Option<String>, stream_id: Option<StreamId>)
        -> anyhow::Result<bool>;
    async fn push(
        &self,
        id: Option<String>,
        stream_id: Option<StreamId>,
        tip: Cid,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl Store for () {
    async fn add(&self, _id: String, _stream_id: &StreamId) -> anyhow::Result<()> {
        Ok(())
    }

    async fn exists(
        &self,
        _id: Option<String>,
        _stream_id: Option<StreamId>,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn push(
        &self,
        _id: Option<String>,
        _stream_id: Option<StreamId>,
        _tip: Cid,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
