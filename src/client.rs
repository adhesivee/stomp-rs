use tokio::net::TcpStream;
use crate::connection::Connection;
use tokio::sync::broadcast::{channel, Sender, Receiver};
use crate::protocol::frame::{Connect, Subscribe};
use std::error::Error;
use crate::protocol::{StompMessage, ClientCommand, ServerCommand};
use tokio::time::{Duration, Instant};
use tokio::time::error::Elapsed;
use tokio::sync::broadcast::error::RecvError;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

pub struct Client {
    connection: Connection,
    sender: Sender<StompMessage<ServerCommand>>,
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
    ConnectionError(Box<dyn Error>)
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
            connection: Connection::new(
                TcpStream::connect(builder.host.clone()).await?,
                sender.clone(),
            ).await,
            sender: sender.clone(),
        };

        client.connection.emit(
            Connect::new("1.2".to_owned(), builder.host)
        ).await?;

        Ok(client)
    }

    pub async fn subscribe(&self, destination: String) -> Result<(), Box<dyn Error>> {
        let subscriber_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        self.connection.emit(
            Subscribe::new(subscriber_id.to_string(), destination.clone())
                .receipt(receipt_id.to_string())
        ).await?;

        let mut receiver = self.sender
            .clone()
            .subscribe();

        Self::process_receipt(&mut receiver, receipt_id.to_string(), destination.clone()).await?;

        Ok(())
    }

    async fn process_receipt(
        receiver: &mut Receiver<StompMessage<ServerCommand>>,
        receipt_id: String,
        destination: String
    ) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        loop {
            match tokio::time::timeout(Duration::from_millis(10), receiver.recv()).await {
                Ok(Ok(StompMessage::Frame(val))) => {
                    match val.command {
                        ServerCommand::Receipt => {
                            if val.headers.contains_key("receipt-id")  && *val.headers.get("receipt-id").unwrap() == receipt_id.to_string(){
                                return Ok(())
                            }
                        }
                        ServerCommand::Error => {
                            if val.headers.contains_key("receipt-id")  && *val.headers.get("receipt-id").unwrap() == receipt_id.to_string(){
                                return Err(Box::new(ClientError::Nack(format!("No received during subscribe of {}", destination))))
                            }
                        }
                        _ => { /* non-relevant frame */ }
                    }
                }
                Ok(Err(cause)) => {
                    return Err(Box::new(ClientError::ConnectionError(Box::new(cause))));
                }
                Ok(_) => { /* ignore, message not relevant for this process */}
                Err(_) => { /* elapsed time check done later */}
            }

            if start.elapsed().as_millis() > 2000 {
                return Err(Box::new(ClientError::ReceiptTimeout("".to_owned())));
            }
        }
    }
}