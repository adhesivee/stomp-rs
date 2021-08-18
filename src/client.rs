mod interceptor;
mod internal;
mod receipt_awaiter;

use crate::client::internal::InternalClient;
use crate::protocol::frame::{Abort, Ack, Begin, Commit, Nack, Send, Subscribe};
use crate::protocol::{Frame, ServerCommand, StompMessage};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

type ReceiptId = String;
type SubscriberId = String;
type ServerStompSender = Sender<StompMessage<ServerCommand>>;
type ServerStompReceiver = Receiver<StompMessage<ServerCommand>>;

pub struct Transaction {
    transaction_id: String,
    inner_client: Arc<InternalClient>,
}

impl Transaction {
    fn new(transaction_id: String, inner_client: Arc<InternalClient>) -> Self {
        Self {
            transaction_id,
            inner_client,
        }
    }

    pub async fn send(&self, send: Send) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .send(send.header("transaction".to_string(), self.transaction_id.clone()))
            .await
    }

    pub async fn ack(&self, ack: Ack) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .ack(ack.transaction(self.transaction_id.clone()))
            .await
    }

    pub async fn nack(&self, nack: Nack) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .nack(nack.transaction(self.transaction_id.clone()))
            .await
    }

    pub async fn commit(&self) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .emit(
                Commit::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into(),
            )
            .await
    }

    pub async fn abort(&self) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .emit(
                Abort::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into(),
            )
            .await
    }
}

pub struct Client {
    inner_client: Arc<InternalClient>,
}

pub struct ClientBuilder {
    host: String,
    heartbeat: Option<(u32, u32)>,
}

impl ClientBuilder {
    pub fn new<A: Into<String>>(host: A) -> Self {
        Self {
            host: host.into(),
            heartbeat: None,
        }
    }

    pub fn heartbeat(mut self, client_interval: u32, server_interval: u32) -> Self {
        self.heartbeat = Some((client_interval, server_interval));

        self
    }
}

#[derive(Debug)]
pub enum ClientError {
    ReceiptTimeout(String),
    Nack(String),
    ConnectionError(Option<Box<dyn Error>>),
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client error")
    }
}

impl Error for ClientError {}

impl Client {
    pub async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let inner_client = InternalClient::connect(builder).await?;

        Ok(Self {
            inner_client: Arc::new(inner_client),
        })
    }

    pub async fn subscribe(
        &self,
        subscribe: Subscribe,
        sender: Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        self.inner_client.subscribe(subscribe, sender).await
    }

    pub async fn send(&self, send: Send) -> Result<(), Box<dyn Error>> {
        self.inner_client.send(send).await
    }

    pub async fn ack(&self, ack: Ack) -> Result<(), Box<dyn Error>> {
        self.inner_client.ack(ack).await
    }

    pub async fn nack(&self, nack: Nack) -> Result<(), Box<dyn Error>> {
        self.inner_client.nack(nack).await
    }

    pub async fn begin(&self) -> Result<Transaction, Box<dyn Error>> {
        let transaction_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        self.inner_client
            .emit(
                Begin::new(transaction_id.to_string())
                    .receipt(receipt_id.to_string())
                    .into(),
            )
            .await?;

        Ok(Transaction {
            transaction_id: transaction_id.to_string(),
            inner_client: Arc::clone(&self.inner_client),
        })
    }
}
