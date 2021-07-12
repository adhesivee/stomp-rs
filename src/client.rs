use tokio::net::TcpStream;
use crate::connection::Connection;
use tokio::sync::broadcast::{channel, Sender};
use crate::protocol::frame::{Connect, Subscribe};
use std::error::Error;
use crate::protocol::{StompMessage, ClientCommand, ServerCommand};
use tokio::time::{Duration, Instant};
use tokio::time::error::Elapsed;
use tokio::sync::broadcast::error::RecvError;
use std::fmt::{Display, Formatter};

pub struct Client {
    connection: Connection,
    sender: Sender<StompMessage<ServerCommand>>
}

pub struct ClientBuilder {
    host: String
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
    ReceiptTimeout(String)
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client error")
    }
}

impl Error for ClientError {

}
impl Client {
    pub async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let (sender, mut receiver) = channel(5);

        let client = Self {
            connection: Connection::new(
                TcpStream::connect(builder.host.clone()).await?,
                sender.clone()
            ).await,
            sender: sender.clone()
        };

        client.connection.emit(
            Connect::new("1.2".to_owned(), builder.host)
        ).await?;

        Ok(client)
    }

    pub async fn subscribe(&self, destination: String) -> Result<(), Box<dyn Error>> {
        self.connection.emit(
            Subscribe::new("".to_owned(), destination)
        ).await?;

        let mut receiver = self.sender
            .clone()
            .subscribe();

        let start = Instant::now();

        loop {
            match tokio::time::timeout(Duration::from_millis(10), receiver.recv()).await {

                Ok(Ok(StompMessage::Frame(val)))  => {
                    if let ServerCommand::Receipt = val.command {
                        // @TODO: Match receipt id
                        return Ok(());
                    } else {

                    }
                }
                Ok(val) => {}
                Err(_) => {}
            }

            if start.elapsed().as_millis() > 2000 {
                return Err(Box::new(ClientError::ReceiptTimeout("".to_owned())));
            }
        }
    }
}