use crate::client::interceptor::{ForwardChannel, Forwarder, InterceptorMessage};
use crate::protocol::{Frame, ServerCommand};
use log::debug;
use std::collections::HashMap;
use tokio::sync::mpsc::{channel, Sender};
use tokio::time::{sleep, Duration};

pub struct SubscriberActor {
    interceptor_sender: Sender<(Forwarder, InterceptorMessage)>,
    subscriber_sender: Sender<SubscriberMessage>,
}

pub enum SubscriberMessage {
    Register {
        subscriber_id: String,
        destination: String,
        sender: Sender<Frame<ServerCommand>>,
    },
    Unregister(String),
}

impl SubscriberActor {
    pub async fn new() -> Self {
        let (interceptor_sender, mut interceptor_receiver): ForwardChannel = channel(16);
        let (subscriber_sender, mut subscriber_receiver) = channel(16);

        let inner_subscriber_sender = subscriber_sender.clone();
        tokio::spawn(async move {
            let mut subscribers: HashMap<String, Sender<Frame<ServerCommand>>> = HashMap::new();

            loop {
                tokio::select! {
                    message = interceptor_receiver.recv() => {
                        process_interceptor(message, &mut subscribers)
                            .await;
                    },
                    message = subscriber_receiver.recv() => {
                        process_subscriber(message, &mut subscribers);
                    }
                    _ = sleep(Duration::from_millis(250)) => {
                        let closed_subscribers: Vec<String> = subscribers.iter()
                            .filter(|entry| entry.1.is_closed())
                            .map(|closed_entry| closed_entry.0.clone())
                            .collect();

                        for closed_subscriber in closed_subscribers.into_iter() {
                            inner_subscriber_sender.send(SubscriberMessage::Unregister(closed_subscriber))
                                .await;
                        }
                    }
                }
            }
        });

        Self {
            interceptor_sender,
            subscriber_sender,
        }
    }

    pub fn subscriber_sender(&self) -> Sender<SubscriberMessage> {
        self.subscriber_sender.clone()
    }

    pub fn interceptor_sender(&self) -> Sender<(Forwarder, InterceptorMessage)> {
        self.interceptor_sender.clone()
    }
}

async fn process_interceptor(
    message: Option<(Forwarder, InterceptorMessage)>,
    subscribers: &mut HashMap<String, Sender<Frame<ServerCommand>>>,
) {
    match message {
        Some((forwarder, InterceptorMessage::BeforeServerReceive(frame))) => {
            if let Some(subscription) = frame.headers.get("subscription") {
                if let Some(sub_sender) = subscribers.get(subscription) {
                    if sub_sender.send(frame.clone()).await.is_err() {
                        debug!("Could not deliver message to subscriber {}", subscription)
                    }
                }
            }

            forwarder
                .proceed(InterceptorMessage::BeforeServerReceive(frame))
                .await;
        }
        Some((forwarder, message)) => {
            forwarder.proceed(message).await;
        }
        _ => {}
    }
}

fn process_subscriber(
    message: Option<SubscriberMessage>,
    subscribers: &mut HashMap<String, Sender<Frame<ServerCommand>>>,
) {
    if let Some(message) = message {
        match message {
            SubscriberMessage::Register {
                subscriber_id,
                destination,
                sender,
            } => {
                subscribers.insert(subscriber_id, sender);
            }
            SubscriberMessage::Unregister(id) => {
                subscribers.remove(&id);
            }
        }
    }
}
