use std::collections::HashMap;

use crate::ceramic::StreamState;

use ceramic_core::StreamId;
use once_cell::sync::Lazy;

static STREAM_STORE: Lazy<StreamStore> = Lazy::new(StreamStore::new);

pub struct StreamStore {
    streams: HashMap<String, StreamState>,
}

impl StreamStore {
    fn new() -> Self {
        StreamStore {
            streams: HashMap::new(),
        }
    }

    pub fn get_instance() -> &'static StreamStore {
        &STREAM_STORE
    }

    pub async fn get_stream(&self, stream_id: &StreamId) -> anyhow::Result<StreamState> {
        match self.streams.get_key_value(&stream_id.to_string()) {
            Some(state) => Ok(state.1.clone()),
            None => anyhow::bail!("stream not found"),
        }
    }

    pub async fn store_stream(&mut self, stream: &StreamState) -> anyhow::Result<()> {
        let stream_id = stream.stream_id()?;
        self.streams.insert(stream_id.to_string(), stream.clone());
        Ok(())
    }
}
