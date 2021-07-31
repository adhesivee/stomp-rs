mod inner;

use tokio::net::TcpStream;
use crate::connection::Connection;
use tokio::sync::mpsc::{channel, Sender};
use crate::protocol::frame::{Connect, Subscribe, Unsubscribe, Send, Begin, Commit, Abort};
use std::error::Error;
use crate::protocol::{StompMessage, ServerCommand, Frame, ClientCommand};
use tokio::time::{Duration, Instant};
use std::fmt::{Display, Formatter};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::client::inner::InnerClient;
use tokio::sync::mpsc::error::SendError;

type ReceiptId = String;
type SubscriberId = String;
type ServerStompSender = Sender<StompMessage<ServerCommand>>;

pub struct Transaction {
    transaction_id: String,
    inner_client: Arc<InnerClient>,
}

impl Transaction {
    fn new(transaction_id: String, inner_client: Arc<InnerClient>) -> Self {
        Self {
            transaction_id,
            inner_client,
        }
    }

    pub async fn send(
        &self,
        destination: String,
        message: String,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<(), Box<dyn Error>> {
        let mut headers = headers.unwrap_or_else(|| &[])
            .to_vec();

        headers.push(("transaction", &self.transaction_id));

        self.inner_client.send(
            destination,
            message,
            Some(&headers[..]),
        ).await
    }

    pub async fn ack(&self, ack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        let mut headers = headers.unwrap_or_else(|| &[])
            .to_vec();

        headers.push(("transaction", &self.transaction_id));

        self.inner_client.ack(
            ack_id,
            Some(&headers[..]),
        ).await
    }

    pub async fn nack(&self, nack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        let mut headers = headers.unwrap_or_else(|| &[])
            .to_vec();

        headers.push(("transaction", &self.transaction_id));

        self.inner_client.nack(
            nack_id,
            Some(&headers[..]),
        ).await
    }

    pub async fn commit(&self) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .emit(
                Commit::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into()
            )
            .await
    }

    pub async fn abort(&self) -> Result<(), Box<dyn Error>> {
        self.inner_client
            .emit(
                Abort::new(self.transaction_id.clone())
                    .receipt(Uuid::new_v4().to_string())
                    .into()
            )
            .await
    }
}

pub struct Client {
    inner_client: Arc<InnerClient>,
}

pub struct ClientBuilder {
    host: String,
    heartbeat: Option<(u32, u32)>
}

impl ClientBuilder {
    pub fn new(host: String) -> Self {
        Self {
            host,
            heartbeat: None
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
        let inner_client = InnerClient::connect(builder).await?;

        Ok(
            Self {
                inner_client: Arc::new(inner_client)
            }
        )
    }

    pub async fn subscribe(
        &self,
        destination: String,
        headers: Option<&[(&str, &str)]>,
        sender: Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        self.inner_client.subscribe(destination, headers, sender).await
    }

    pub async fn send(
        &self,
        destination: String,
        message: String,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<(), Box<dyn Error>> {
        self.inner_client.send(destination, message, headers).await
    }

    pub async fn ack(&self, ack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        self.inner_client.ack(ack_id, headers).await
    }

    pub async fn nack(&self, nack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        self.inner_client.nack(nack_id, headers).await
    }

    pub async fn begin(&self) -> Result<Transaction, Box<dyn Error>> {
        let transaction_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        self.inner_client
            .emit(
                Begin::new(transaction_id.to_string())
                    .receipt(receipt_id.to_string())
                    .into()
            )
            .await?
        ;

        Ok(Transaction {
            transaction_id: transaction_id.to_string(),
            inner_client: Arc::clone(&self.inner_client),
        })
    }
}