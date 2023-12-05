use anyhow::Context;
use ceramic_core::{Cid, StreamId};
use dataverse_ceramic::{
    event::{Event, EventValue, EventsLoader, EventsUploader},
    http, Ceramic, StreamAnchorRequester,
};
use dataverse_core::stream::StreamStore;
use dataverse_file_system::file::StreamFileLoader;

use crate::Client;

impl StreamFileLoader for Client {}

#[async_trait::async_trait]
impl EventsUploader for Client {
    async fn upload_event(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()> {
        if let EventValue::Signed(signed) = &commit.value {
            if signed.is_gensis() {
                let (ceramic, stream_id) = (ceramic.clone(), stream_id.clone());
                tokio::spawn(async move {
                    let http_operator = http::Client::new();
                    if let Err(err) = http_operator.request_anchor(&ceramic, &stream_id).await {
                        log::error!("failed to upload event: {}", err);
                    };
                });
            }
        }
        self.operator.upload_event(ceramic, stream_id, commit).await
    }
}

#[async_trait::async_trait]
impl EventsLoader for Client {
    async fn load_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>> {
        let tip = match tip {
            Some(tip) => tip,
            None => {
                self.load_stream(stream_id)
                    .await?
                    .context(format!("stream not found: {}", stream_id))?
                    .tip
            }
        };
        self.operator
            .load_events(ceramic, stream_id, Some(tip))
            .await
    }
}
