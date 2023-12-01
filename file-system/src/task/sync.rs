use dataverse_core::stream::Stream;
use fang::async_trait;
use fang::asynk::async_queue::AsyncQueueable;
use fang::serde::{Deserialize, Serialize};
use fang::typetag;
use fang::AsyncRunnable;
use fang::FangError;

#[derive(Serialize, Deserialize)]
#[serde(crate = "fang::serde")]
pub struct Task {
    pub stream: Stream,
}

#[async_trait]
#[typetag::serde]
impl AsyncRunnable for Task {
    async fn run(&self, queue: &mut dyn AsyncQueueable) -> Result<(), FangError> {
        todo!("implement Task::run")
    }
}
