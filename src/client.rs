mod handler;
mod interceptor;
mod internal;

use crate::client::internal::InternalClient;
use crate::protocol::frame::{Abort, Ack, Begin, Commit, Nack, Send, Subscribe};
use crate::protocol::{Frame, ServerCommand};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use uuid::Uuid;

type ReceiptId = String;

pub struct Transaction {
    transaction_id: String,
    internal_client: Arc<InternalClient>,
}

impl Transaction {
    fn new(transaction_id: String, internal_client: Arc<InternalClient>) -> Self {
        Self {
            transaction_id,
            internal_client,
        }
    }

    pub async fn send(&self, send: Send) -> Result<SendReceipt, Box<dyn Error>> {
        self.internal_client
            .send(send.header("transaction", self.transaction_id.clone()))
            .await
    }

    pub async fn ack(&self, ack: Ack) -> Result<(), Box<dyn Error>> {
        self.internal_client
            .ack(ack.transaction(self.transaction_id.clone()))
            .await
    }

    pub async fn nack(&self, nack: Nack) -> Result<(), Box<dyn Error>> {
        self.internal_client
            .nack(nack.transaction(self.transaction_id.clone()))
            .await
    }

    pub async fn commit(&self) -> Result<(), Box<dyn Error>> {
        self.internal_client
            .emit(
                Commit::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into(),
            )
            .await
    }

    pub async fn abort(&self) -> Result<(), Box<dyn Error>> {
        self.internal_client
            .emit(
                Abort::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into(),
            )
            .await
    }
}

pub struct Client {
    internal_client: Arc<InternalClient>,
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

pub struct SendReceipt {
    pub receipt_id: String,
    pub notify: Arc<Notify>,
}

impl Client {
    pub async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let internal_client = InternalClient::connect(builder).await?;

        Ok(Self {
            internal_client: Arc::new(internal_client),
        })
    }

    pub async fn subscribe(
        &self,
        subscribe: Subscribe,
        sender: Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        self.internal_client.subscribe(subscribe, sender).await
    }

    pub async fn send(&self, send: Send) -> Result<SendReceipt, Box<dyn Error>> {
        self.internal_client.send(send).await
    }

    pub async fn ack(&self, ack: Ack) -> Result<(), Box<dyn Error>> {
        self.internal_client.ack(ack).await
    }

    pub async fn nack(&self, nack: Nack) -> Result<(), Box<dyn Error>> {
        self.internal_client.nack(nack).await
    }

    pub async fn begin(&self) -> Result<Transaction, Box<dyn Error>> {
        let transaction_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        self.internal_client
            .emit(
                Begin::new(transaction_id.to_string())
                    .receipt(receipt_id.to_string())
                    .into(),
            )
            .await?;

        Ok(Transaction {
            transaction_id: transaction_id.to_string(),
            internal_client: Arc::clone(&self.internal_client),
        })
    }
}

impl Frame<ServerCommand> {
    pub fn ack(&self) -> Option<Ack> {
        if let ServerCommand::Message = self.command {
            if let Some(ack) = self.headers.get("ack") {
                return Some(Ack::new(ack));
            }
        }

        None
    }

    pub fn nack(&self) -> Option<Nack> {
        if let ServerCommand::Message = self.command {
            if let Some(ack) = self.headers.get("ack") {
                return Some(Nack::new(ack));
            }
        }

        None
    }
}
