use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::AsyncRunnable;

#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
struct AsyncTask {
    pub number: u16,
}
