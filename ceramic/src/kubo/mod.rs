pub mod cache;
pub mod message;
pub mod pubsub;
pub mod store;
pub mod task;

pub use cache::Cached;
pub use store::Store;

use std::sync::Arc;

use ceramic_core::{Cid, StreamId};
use ceramic_kubo_rpc_server::{
    models, ApiNoContext, BlockGetPostResponse, ContextWrapperExt, PubsubPubPostResponse,
    PubsubSubPostResponse,
};
use futures::StreamExt;
use futures_util::FutureExt;
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
pub trait MessageSubscriber {
    async fn subscribe(&self, store: Arc<dyn store::Store>, network: Network)
        -> anyhow::Result<()>;

    async fn message_handler(store: Arc<dyn store::Store>, msg: Message) -> anyhow::Result<()> {
        match msg {
            Message::Response { id, tips } => {
                if store.exists(Some(id.clone()), None).await.unwrap_or(true) {
                    for (stream_id, tip) in tips {
                        let push = store
                            .push(Some(id.clone()), Some(stream_id.parse()?), tip.parse()?)
                            .await;
                        if let Err(err) = push {
                            log::error!("store push error: {}", err)
                        }
                    }
                };
            }
            Message::Update {
                stream,
                tip,
                model: _,
            } => {
                if store
                    .exists(None, Some(stream.parse()?))
                    .await
                    .unwrap_or(true)
                {
                    if let Err(err) = store.push(None, Some(stream.parse()?), tip.parse()?).await {
                        log::error!("store push error: {}", err)
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
            let events_handle = tokio::task::spawn(
                body.for_each_concurrent(None, move |event| {
                    let store = Arc::clone(&store);
                    async move {
                        if let std::result::Result::Ok(data) = event {
                            let msg_resp: MessageResponse =
                                serde_json::from_slice(&data).expect("should be json message");
                            let msg_data = multibase::decode(msg_resp.data).unwrap().1;
                            if let Ok(msg) = serde_json::from_slice::<Message>(&msg_data) {
                                tracing::info!(?network, ?msg, "kubo sub receive msg");
                                if let Err(err) = Self::message_handler(store, msg.clone()).await {
                                    tracing::error!(
                                        ?network,
                                        ?msg,
                                        "message handler error: {}",
                                        err
                                    )
                                };
                            }
                        };
                    }
                })
                .map(|_| ()),
            );

            events_handle.await?;
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

        let state = StreamState::new(stream_id.r#type.int_value(), commits.clone())?;
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
