pub mod cache;
pub mod message;
pub mod pubsub;
pub mod store;
pub mod task;

use bytes::Bytes;
pub use cache::Cached;
pub use store::Store;

use std::sync::Arc;

use ceramic_core::{Cid, StreamId};
use ceramic_kubo_rpc_server::{
    models, ApiNoContext, BlockGetPostResponse, ContextWrapperExt, PubsubPubPostResponse,
    PubsubSubPostResponse,
};
use futures::StreamExt;
use int_enum::IntEnum;
use serde_json::json;
use swagger::{AuthData, ByteArray, ContextBuilder, EmptyContext, Push, XSpanIdString};

use crate::{
    event::{self, Event, EventsLoader, EventsUploader, ToCid},
    kubo::message::MessageResponse,
    network::Network,
    Ceramic, StreamLoader, StreamState,
};

use self::{message::message_hash, pubsub::Message};

pub type ClientContext = swagger::make_context_ty!(
    ContextBuilder,
    EmptyContext,
    Option<AuthData>,
    XSpanIdString
);

pub type Client = Box<dyn ApiNoContext<ClientContext> + Send + Sync>;

pub fn new(base_path: &str) -> Client {
    let context: ClientContext = swagger::make_context!(
        ContextBuilder,
        EmptyContext,
        None as Option<AuthData>,
        XSpanIdString::default()
    );

    // Using HTTP
    let client = Box::new(
        ceramic_kubo_rpc_server::Client::try_new_http(&base_path)
            .expect("Failed to create HTTP client"),
    );
    Box::new(client.with_context(context))
}

#[async_trait::async_trait]
pub trait CidLoader {
    async fn load_cid(&self, cid: &Cid) -> anyhow::Result<Vec<u8>>;
}

#[async_trait::async_trait]
impl CidLoader for Client {
    async fn load_cid(&self, cid: &Cid) -> anyhow::Result<Vec<u8>> {
        let result;
        let timeout = Some("1s".into());
        let res = self.block_get_post(cid.to_string(), timeout, None).await?;

        match res {
            BlockGetPostResponse::Success(bytes) => {
                result = bytes.to_vec();
            }
            BlockGetPostResponse::BadRequest(err) => {
                anyhow::bail!("bad request: {:?}", err);
            }
            BlockGetPostResponse::InternalError(err) => {
                anyhow::bail!("internal error: {:?}", err);
            }
        }

        Ok(result)
    }
}

#[async_trait::async_trait]
pub trait TipQueryer {
    async fn query_last_tip(&self, network: Network, stream_id: &StreamId) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl TipQueryer for Client {
    async fn query_last_tip(&self, network: Network, stream_id: &StreamId) -> anyhow::Result<()> {
        let stream_id_str = stream_id.to_string();
        let id = message_hash(1, stream_id_str.to_string())?;
        let msg = json!({
            "typ": 1,
            "id": id,
            "stream": stream_id_str,
        });
        let file = serde_json::to_vec(&msg)?;
        let topic = network.kubo_topic();
        let res = self
            .pubsub_pub_post(topic.clone(), swagger::ByteArray(file))
            .await?;
        if let PubsubPubPostResponse::BadRequest(resp) = res {
            anyhow::bail!(resp.message);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait MessageSubscriber: MessageResponsePublisher {
    async fn subscribe(&self, store: Arc<dyn store::Store>, network: Network)
        -> anyhow::Result<()>;

    async fn kubo_message_handler(
        &self,
        network: Network,
        store: Arc<dyn store::Store>,
        event: Result<Bytes, Box<dyn std::error::Error + Send + Sync>>,
    ) -> () {
        let msg_resp = match event {
            Ok(data) => match serde_json::from_slice::<MessageResponse>(&data) {
                Ok(msg) => msg,
                Err(err) => {
                    let data = String::from_utf8_lossy(&data);
                    tracing::error!(?data, "failed to decode as MessageResponse: {}", err);
                    return;
                }
            },
            Err(err) => {
                tracing::error!("kubo sub error: {}", err);
                return;
            }
        };

        if let Ok((_, msg_data)) = multibase::decode(msg_resp.data) {
            if let Ok(msg) = serde_json::from_slice::<Message>(&msg_data) {
                tracing::info!(?network, ?msg, "kubo sub receive msg");
                if let Err(err) = self
                    .ceramic_message_handler(network, store, msg.clone())
                    .await
                {
                    tracing::error!(?network, ?msg, "ceramic message handler error: {}", err)
                };
            }
        };
    }

    async fn ceramic_message_handler(
        &self,
        network: Network,
        store: Arc<dyn store::Store>,
        msg: Message,
    ) -> anyhow::Result<()> {
        match msg {
            Message::Query { id, stream } => {
                let stream_id: StreamId = stream.parse()?;
                if let Some(tip) = store.get(Some(id.clone()), Some(stream_id.clone())).await? {
                    tracing::info!(?network, ?id, ?stream, ?tip, "query stored response");
                    if let Err(err) = self.publish_response(&network, &id, &stream_id, &tip).await {
                        tracing::error!(?network, ?id, ?stream, "publish response error: {}", err)
                    }
                }
            }
            Message::Response { id, tips } => {
                if let Some(_) = store.get(Some(id.clone()), None).await? {
                    for (stream_id, tip) in tips {
                        let push = store
                            .push(Some(id.clone()), Some(stream_id.parse()?), tip.parse()?)
                            .await;
                        if let Err(err) = push {
                            tracing::error!(stream_id = id, "store push error: {}", err)
                        }
                    }
                };
            }
            Message::Update {
                stream,
                tip,
                model: _,
            } => {
                if let Some(tip_old) = store.get(None, Some(stream.parse()?)).await? {
                    if tip_old.to_string() == tip {
                        tracing::info!(?network, ?stream, ?tip, "update tip not changed");
                        return Ok(());
                    }
                    if let Err(err) = store.push(None, Some(stream.parse()?), tip.parse()?).await {
                        tracing::error!(stream, tip, "store push error: {}", err)
                    }
                }
            }
            _ => (),
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl MessageSubscriber for Client {
    async fn subscribe(
        &self,
        store: Arc<dyn store::Store>,
        network: Network,
    ) -> anyhow::Result<()> {
        let sub = self.pubsub_sub_post(network.kubo_topic()).await?;

        if let PubsubSubPostResponse::Success(body) = sub {
            let store = Arc::clone(&store);
            let handler = body.for_each_concurrent(None, move |event| {
                self.kubo_message_handler(network, store.clone(), event)
            });
            handler.await;
            return Ok(());
        }
        anyhow::bail!("subscribe failed")
    }
}

#[async_trait::async_trait]
pub trait MessageUpdatePublisher {
    async fn publish_update(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: &Cid,
        model: &StreamId,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl MessageUpdatePublisher for Client {
    async fn publish_update(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        tip: &Cid,
        model: &StreamId,
    ) -> anyhow::Result<()> {
        let msg = json!({
            "typ": 0,
            "stream": stream_id.to_string(),
            "tip": tip.to_string(),
            "model": model.to_string(),
        });
        let file = serde_json::to_vec(&msg)?;
        let res = self
            .pubsub_pub_post(ceramic.network.kubo_topic(), swagger::ByteArray(file))
            .await?;
        if let PubsubPubPostResponse::BadRequest(resp) = res {
            anyhow::bail!(resp.message);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait MessageResponsePublisher {
    async fn publish_response(
        &self,
        network: &Network,
        id: &String,
        stream_id: &StreamId,
        tip: &Cid,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl MessageResponsePublisher for Client {
    async fn publish_response(
        &self,
        network: &Network,
        id: &String,
        stream_id: &StreamId,
        tip: &Cid,
    ) -> anyhow::Result<()> {
        let msg = json!({
            "typ": 2,
            "id": id,
            "tips": {
                stream_id.to_string(): tip.to_string(),
            }
        });
        let file = serde_json::to_vec(&msg)?;
        let res = self
            .pubsub_pub_post(network.kubo_topic(), swagger::ByteArray(file))
            .await?;
        if let PubsubPubPostResponse::BadRequest(resp) = res {
            anyhow::bail!("failed to post msg to kubo: {}", resp.message);
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl EventsUploader for Client {
    async fn upload_event(
        &self,
        _ceramic: &Ceramic,
        _stream_id: &StreamId,
        commit: Event,
    ) -> anyhow::Result<()> {
        let mhtype = Some(models::Multihash::Sha2256);
        match commit.value {
            event::EventValue::Signed(signed) => {
                if let Some(cacao_block) = signed.cacao_block {
                    let file = ByteArray(cacao_block);
                    let _ = self.block_put_post(file, None, mhtype, None).await?;
                }
                if let Some(linked_block) = signed.linked_block {
                    let file = ByteArray(linked_block);
                    let _ = self.block_put_post(file, None, mhtype, None).await?;
                }
                let file = ByteArray(signed.jws.to_vec()?);
                let _ = self.block_put_post(file, None, mhtype, None).await?;
            }
            // anchor commit generate by ceramic node default
            // don't need to upload it
            event::EventValue::Anchor(_) => {}
        }
        Ok(())
    }

    async fn upload_events(
        &self,
        ceramic: &Ceramic,
        stream_id: &StreamId,
        commits: Vec<Event>,
    ) -> anyhow::Result<()> {
        let tip = match commits.last() {
            Some(commit) => commit.cid,
            None => anyhow::bail!("input commits of {} is empty", stream_id),
        };

        let state = StreamState::make(stream_id.r#type.int_value(), commits.clone()).await?;
        let model = state.must_model()?;

        for commit in commits {
            self.upload_event(ceramic, stream_id, commit).await?;
        }

        self.publish_update(ceramic, stream_id, &tip, &model)
            .await?;
        Ok(())
    }
}

impl StreamLoader for Client {}

#[async_trait::async_trait]
impl<T: CidLoader + Send + Sync> EventsLoader for T {
    async fn load_events(
        &self,
        _ceramic: &Ceramic,
        _stream_id: &StreamId,
        tip: Option<Cid>,
    ) -> anyhow::Result<Vec<Event>> {
        let mut commits = Vec::new();
        let mut cid = match tip {
            Some(tip) => tip,
            None => {
                log::warn!("kubo not support query tip, should input tip");
                todo!("query latest tip of stream")
            }
        };
        loop {
            let bytes = self.load_cid(&cid).await?;
            let mut commit = event::Event::decode(cid, bytes.to_vec())?;
            match &mut commit.value {
                event::EventValue::Signed(signed) => {
                    signed.linked_block = Some(self.load_cid(&signed.payload_link()?).await?);
                    signed.cacao_block = Some(self.load_cid(&signed.cap()?).await?);
                }
                event::EventValue::Anchor(anchor) => {
                    anchor.proof_block = Some(self.load_cid(&anchor.proof).await?)
                }
            }
            commits.insert(0, commit.clone());
            match commit.prev()? {
                Some(prev) => cid = prev,
                None => break,
            };
        }
        Ok(commits)
    }
}
