use crate::client::interceptor::{ConnectionHook};
use crate::protocol::{Frame, ServerCommand};
use log::debug;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use async_trait::async_trait;

type Subscribers = Arc<Mutex<HashMap<String, Sender<Frame<ServerCommand>>>>>;

#[derive(Clone)]
pub struct SubscriberHandler {
    subscribers: Subscribers,
}

impl SubscriberHandler {
    pub async fn new() -> Self {
        let subscribers: Subscribers =  Default::default();
        let subscribers_clone =  Arc::clone(&subscribers);

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;

                let mut locked_subscribers = subscribers_clone.lock().await;

                let remove_subscribers: Vec<_> = locked_subscribers.iter()
                    .filter(|sub| sub.1.is_closed())
                    .map(|sub| sub.0.clone())
                    .collect();

                for subscriber in remove_subscribers {
                    locked_subscribers.remove(&subscriber);
                }
            }
        });
        Self {
            subscribers
        }
    }

    pub async fn register(
        &self,
        subscriber_id: impl Into<String>,
        _destination: impl Into<String>,
        sender: Sender<Frame<ServerCommand>>
    ) {
        self.subscribers.lock()
            .await
            .insert(subscriber_id.into(), sender);
    }
}

#[async_trait]
impl ConnectionHook for SubscriberHandler {
    async fn before_receive(&self, frame: &Frame<ServerCommand>) {
        if let Some(subscription) = frame.headers.get("subscription") {
            let lock_subscribers = self.subscribers.lock().await;
            if let Some(sub_sender) = lock_subscribers.get(subscription) {
                if sub_sender.send(frame.clone()).await.is_err() {
                    debug!("Could not deliver message to subscriber {}", subscription)
                }
            }
        }
    }
}