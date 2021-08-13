use crate::protocol::{FrameParser, ServerCommand, StompMessage, ClientCommand, Frame, ParseError};
use tokio::sync::mpsc::{Sender, channel};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::SendError;
use tokio::io::{ErrorKind, AsyncWriteExt, AsyncReadExt};
use crate::protocol::BNF_LF;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::debug;
use std::fmt::{Display, Formatter};
use std::error::Error;

#[derive(Debug)]
pub enum ClosingReason {
    ParseError(ParseError),
    ConnectionError(std::io::Error)
}

#[derive(Debug)]
pub enum ConnectionError {
    Closing(ClosingReason)
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client error")
    }
}

impl Error for ConnectionError {}

pub struct Connection {
    client_sender: Sender<StompMessage<ClientCommand>>,
    close_sender: Sender<()>,
    is_closed: Arc<Mutex<bool>>,
}

impl Connection {
    pub async fn new(
        mut tcp_stream: TcpStream,
        server_sender: Sender<Result<StompMessage<ServerCommand>, ConnectionError>>,
    ) -> Self {
        let (sender, mut receiver) = channel(5);

        let (close_sender, mut close_receiver) = channel(1);
        let inner_close_sender = close_sender.clone();
        let is_closed = Arc::new(Mutex::new(false));
        let inner_is_closed = Arc::clone(&is_closed);

        tokio::spawn(async move {
            let mut msg = vec![0; 8096];
            let mut parser: FrameParser<ServerCommand> = FrameParser::new();
            let mut closing = false;


            loop {
                tokio::select! {
                    frame = receiver.recv(), if !closing => {
                         if let Some(message) = frame {
                            match message {
                                StompMessage::Frame(frame) => tcp_stream.write_all(&frame.to_bytes()).await.unwrap(),
                                StompMessage::Ping => tcp_stream.write_u8(BNF_LF).await.unwrap()
                            }

                            tcp_stream.flush().await.unwrap();
                        }
                    },
                    read = tcp_stream.read(&mut msg), if !closing => {
                        match read {
                            Ok(n) => {
                                match parser.parse(&msg[..n]) {
                                    Ok(messages) => {
                                        for message in messages {
                                            debug!("Message received {:?}", message.clone());
                                            server_sender.send(Ok(message)).await.unwrap();
                                        }
                                    }
                                    Err(e) => {
                                        debug!("Parsing error, closing {:?}", e);
                                        server_sender.send(Err(ConnectionError::Closing(ClosingReason::ParseError(e))));
                                        inner_close_sender.send(()).await.unwrap();
                                        closing = true;
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                            Err(e) => {
                                debug!("Connection error, closing {:?}", e);
                                server_sender.send(Err(ConnectionError::Closing(ClosingReason::ConnectionError(e))));
                                inner_close_sender.send(()).await.unwrap();
                                closing = true;
                            }
                        }
                    }
                    _ = close_receiver.recv() => {
                        debug!("Closing connection");
                        tcp_stream.shutdown()
                            .await
                            .unwrap();

                        receiver.close();

                        let mut guard = inner_is_closed.lock().await;
                        *guard = true;
                        break;
                    }
                }
                ;
            }
        });


        Connection {
            client_sender: sender,
            close_sender,
            is_closed,
        }
    }

    pub async fn is_closed(&self) -> bool {
        *self.is_closed
            .lock()
            .await
    }
    pub async fn emit<T: Into<Frame<ClientCommand>>>(&self, frame: T) -> Result<(), SendError<StompMessage<ClientCommand>>> {
        self.client_sender
            .send(StompMessage::Frame(frame.into()))
            .await
    }

    pub async fn heartbeat(&self) -> Result<(), SendError<StompMessage<ClientCommand>>> {
        self.client_sender
            .send(StompMessage::Ping)
            .await
    }

    pub async fn close(&self) {
        self.close_sender.send(()).await;
    }
}