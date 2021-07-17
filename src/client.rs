use tokio::net::TcpStream;
use crate::connection::Connection;
use tokio::sync::mpsc::{channel, Sender};
use crate::protocol::frame::{Connect, Subscribe, Unsubscribe, Send};
use std::error::Error;
use crate::protocol::{StompMessage, ServerCommand, Frame};
use tokio::time::{Duration, Instant};
use std::fmt::{Display, Formatter};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;

type ReceiptId = String;
type SubscriberId = String;
type ServerStompSender = Sender<StompMessage<ServerCommand>>;

pub struct Client {
    connection: Arc<Connection>,
    sender: ServerStompSender,
    pending_receipts: Arc<Mutex<HashMap<ReceiptId, ServerStompSender>>>,
    subscribers: Arc<Mutex<HashMap<SubscriberId, ServerStompSender>>>
}

pub struct ClientBuilder {
    host: String,
}

impl ClientBuilder {
    pub fn new(host: String) -> Self {
        Self {
            host
        }
    }
}

#[derive(Debug)]
pub enum ClientError {
    ReceiptTimeout(String),
    Nack(String),
    ConnectionError(Box<dyn Error>),
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client error")
    }
}

impl Error for ClientError {}

impl Client {
    pub async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let (sender, mut receiver) = channel(5);

        let client = Self {
            connection: Arc::new(Connection::new(
                TcpStream::connect(builder.host.clone()).await?,
                sender.clone(),
            ).await),
            sender,
            pending_receipts: Arc::new(Default::default()),
            subscribers: Arc::new(Default::default())
        };

        let subscribers = Arc::clone(&client.subscribers);
        let pending_receipts = Arc::clone(&client.pending_receipts);
        tokio::spawn(async move {
            loop {
                match receiver.recv().await {
                    Some(StompMessage::Frame(frame)) => {
                        if let Some(receipt_id) = frame.headers.get("receipt-id") {
                            let lock = pending_receipts.lock().await;
                            if let Some(pending_sender) = lock.get(receipt_id) {
                                pending_sender.send(StompMessage::Frame(frame.clone())).await.unwrap();
                            }
                            drop(lock);
                        }

                        if let Some(subscription) = frame.headers.get("subscription") {
                            let lock = subscribers.lock().await;
                            if let Some(sub_sender) = lock.get(subscription) {
                                sub_sender.send(StompMessage::Frame(frame.clone())).await.unwrap();
                            }
                            drop(lock);
                        }
                        // if frame.headers.contains_key("receipt-id") && *val.headers.get("receipt-id").unwrap() == receipt_id.as_str() {
                    }
                    Some(StompMessage::Ping) => {}
                    None => {}
                }
            }
        });

        client.connection.emit(
            Connect::new("1.2".to_owned(), builder.host)
        ).await?;

        Ok(client)
    }

    pub async fn subscribe(
        &self,
        destination: String,
        headers: Option<&[(&str, &str)]>,
        sender: tokio::sync::mpsc::Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        let subscriber_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        let subscribe = headers.unwrap_or_else(|| &[])
            .iter()
            .fold(
                Subscribe::new(subscriber_id.to_string(), destination.clone()),
                |subscribe, header| subscribe.header(header.0.to_string(), header.1.to_string()),
            )
            .receipt(receipt_id.to_string());

        self.connection.emit(subscribe).await?;

        self.await_receipt(receipt_id.to_string(), destination.clone()).await?;

        let sub_connection = Arc::clone(&self.connection);

        let (sub_sender, receiver) = channel(5);
        let mut lock = self.subscribers.lock().await;
        lock.insert(subscriber_id.to_string(), sub_sender);
        drop(lock);

        tokio::spawn(async move {
            let mut sub_recv = receiver;

            while let Some(message) = sub_recv.recv().await {
                if let StompMessage::Frame(frame) = message {
                    let subscription_header = frame.headers.get("subscription");
                    let destination_header = frame.headers.get("destination");

                    if subscription_header.is_some() && subscription_header.unwrap() == &subscriber_id.to_string() &&
                        destination_header.is_some() && destination_header.unwrap() == &destination &&
                        sender.send(frame).await.is_err()
                    {
                        break;
                    }
                }
            }

            sub_connection.emit(
                Unsubscribe::new(subscriber_id.to_string())
            ).await.unwrap();
        });
        Ok(())
    }

    pub async fn send(&self, destination: String, message: String) -> Result<(), Box<dyn Error>> {
        let receipt_id = Uuid::new_v4();

        self.connection.emit(
            Send::new(destination.clone())
                .receipt(receipt_id.to_string())
                .body(message)
        ).await?;

        self.await_receipt(receipt_id.to_string(), destination)
            .await?;

        Ok(())
    }

    async fn await_receipt(
        &self,
        receipt_id: String,
        destination: String,
    ) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        let (sender, mut receiver) = channel(1);

        let mut lock = self.pending_receipts.lock().await;
        lock.insert(receipt_id.clone(), sender);
        drop(lock);

        loop {
            match tokio::time::timeout(Duration::from_millis(10), receiver.recv()).await {
                Ok(Some(StompMessage::Frame(val))) => {
                    match val.command {
                        ServerCommand::Receipt => {
                            if val.headers.contains_key("receipt-id") && *val.headers.get("receipt-id").unwrap() == receipt_id.as_str() {
                                return Ok(());
                            }
                        }
                        ServerCommand::Error => {
                            if val.headers.contains_key("receipt-id") && *val.headers.get("receipt-id").unwrap() == receipt_id.as_str() {
                                return Err(Box::new(ClientError::Nack(format!("No received during subscribe of {}", destination))));
                            }
                        }
                        _ => { /* non-relevant frame */ }
                    }
                }
                Ok(_) => { /* ignore, message not relevant for this process */ }
                Err(_) => { /* elapsed time check done later */ }
            }

            if start.elapsed().as_millis() > 2000 {
                return Err(Box::new(ClientError::ReceiptTimeout("".to_owned())));
            }
        }
    }
}