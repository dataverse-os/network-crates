pub mod store;

use std::sync::Arc;

use ceramic_core::StreamId;
use ceramic_kubo_rpc_server::{
    ApiNoContext, ContextWrapperExt, PubsubPubPostResponse, PubsubSubPostResponse,
};
use futures::StreamExt;
use futures_util::FutureExt;
use serde_json::json;
use swagger::{AuthData, ContextBuilder, EmptyContext, Push, XSpanIdString};
use tokio::task;

use crate::{
    message::{message_hash, MessageResponse},
    pubsub::Message,
};

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
pub trait TipQueryer {
    async fn query_last_tip(
        &self,
        store: Arc<Box<dyn store::Store>>,
        network: String,
        stream_id: &StreamId,
    ) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl TipQueryer for Client {
    async fn query_last_tip(
        &self,
        store: Arc<Box<dyn store::Store>>,
        network: String,
        stream_id: &StreamId,
    ) -> anyhow::Result<()> {
        let stream_id_str = stream_id.to_string();
        let id = message_hash(1, stream_id_str.to_string())?;
        let msg = json!({
            "typ": 1,
            "id": id,
            "stream": stream_id_str,
        });
        let file = serde_json::to_vec(&msg)?;
        let topic = multibase::encode(multibase::Base::Base64Url, network);
        let res = self
            .pubsub_pub_post(topic.clone(), swagger::ByteArray(file))
            .await?;
        if let PubsubPubPostResponse::BadRequest(resp) = res {
            anyhow::bail!(resp.message);
        }
        store.add(id, stream_id).await
    }
}

#[async_trait::async_trait]
pub trait MessageSubscriber {
    async fn subscribe(
        &self,
        store: Arc<Box<dyn store::Store>>,
        topic: String,
    ) -> anyhow::Result<()>;

    async fn message_handler(
        store: Arc<Box<dyn store::Store>>,
        msg: Message,
    ) -> anyhow::Result<()> {
        match msg {
            Message::Response { id, tips } => {
                if store.exists(Some(id.clone()), None).await.unwrap_or(true) {
                    for (stream_id, tip) in tips {
                        if let Err(err) = store
                            .push(Some(id.clone()), Some(stream_id.parse()?), tip.parse()?)
                            .await
                        {
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
        store: Arc<Box<dyn store::Store>>,
        topic: String,
    ) -> anyhow::Result<()> {
        let sub = self.pubsub_sub_post(topic).await?;

        if let PubsubSubPostResponse::Success(body) = sub {
            let store = Arc::clone(&store);
            let events_handle = task::spawn(
                body.for_each_concurrent(None, move |event| {
                    let store = Arc::clone(&store);
                    async move {
                        if let std::result::Result::Ok(data) = event {
                            let msg_resp: MessageResponse =
                                serde_json::from_slice(&data).expect("should be json message");
                            let msg_data = multibase::decode(msg_resp.data).unwrap().1;
                            if let Ok(msg) = serde_json::from_slice(&msg_data) {
                                if let Err(err) = Self::message_handler(store, msg).await {
                                    log::error!("message handler error: {}", err)
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
