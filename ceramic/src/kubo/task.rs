use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct MessageHandler {
    pub msg: Vec<u8>,
}

#[async_trait]
#[typetag::serde]
impl AsyncRunnable for MessageHandler {
    async fn run(&self, _queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        Ok(())
    }
}
