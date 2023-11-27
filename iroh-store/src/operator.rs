use ceramic_core::StreamId;
use dataverse_ceramic::{network::Network, EventsPublisher, StreamState};
use dataverse_core::{
    store::dapp::ModelStore,
    stream::{Stream, StreamLoader, StreamOperator, StreamPublisher},
};

use crate::Client;

#[async_trait::async_trait]
impl StreamOperator for Client {}

#[async_trait::async_trait]
impl StreamLoader for Client {
    async fn load_stream(
        &self,
        _ceramic: &String,
        stream_id: &StreamId,
    ) -> anyhow::Result<StreamState> {
        let stream = self.load_stream(stream_id).await?;
        self.load_stream_state(&stream).await
    }

    async fn load_streams(
        &self,
        account: &Option<String>,
        _ceramic: &String,
        model_id: &StreamId,
    ) -> anyhow::Result<Vec<StreamState>> {
        let states = self.list_stream_states_in_model(model_id).await?;
        if let Some(account) = account {
            let mut streams = Vec::new();
            for state in states {
                if state.controllers().contains(&account) {
                    streams.push(state);
                }
            }
            return Ok(streams);
        } else {
            return Ok(states);
        }
    }
}

#[async_trait::async_trait]
impl StreamPublisher for Client {
    async fn publish_all_streams(&self) -> anyhow::Result<()> {
        let streams = self.list_all_streams().await?;
        for stream in streams {
            let commits = self.load_commits(&stream.tip).await?;
            if stream.published == commits.len() {
                continue;
            }
            self.publish_stream(stream).await?;
        }
        Ok(())
    }

    async fn publish_stream(&self, mut stream: Stream) -> anyhow::Result<()> {
        let model_store = ModelStore::get_instance();
        let ceramic = model_store.get_dapp_ceramic(&stream.dapp_id).await?;
        let stream_id = stream.stream_id()?;
        let events = self.load_commits(&stream.tip).await?;
        let ceramic = dataverse_ceramic::http::Client::init(&ceramic)?;
        stream.published = events.len();
        ceramic
            .publish_events(Network::Mainnet, &stream_id, events)
            .await?;
        Ok(())
    }
}
