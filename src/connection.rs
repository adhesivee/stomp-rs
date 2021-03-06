use crate::protocol::BNF_LF;
use crate::protocol::{ClientCommand, Frame, FrameParser, ParseError, ServerCommand, StompMessage};
use log::debug;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ErrorKind};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::Mutex;

#[derive(Debug)]
pub enum ClosingReason {
    ParseError(ParseError),
    ConnectionError(std::io::Error),
    Shutdown,
}

#[derive(Debug)]
pub enum ConnectionError {
    Closing(ClosingReason),
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client error")
    }
}

impl Error for ConnectionError {}

pub struct Connection {
    client_sender: Sender<StompMessage<ClientCommand>>,
    server_sender: Sender<Result<StompMessage<ServerCommand>, ConnectionError>>,
    close_sender: Sender<()>,
    is_closed: Arc<AtomicBool>
}

impl Connection {
    pub async fn new(
        mut tcp_stream: TcpStream,
        server_sender: Sender<Result<StompMessage<ServerCommand>, ConnectionError>>,
    ) -> Self {
        let (sender, mut receiver) = channel(32);

        let (close_sender, mut close_receiver) = channel(1);
        let inner_close_sender = close_sender.clone();
        let is_closed = Arc::new(AtomicBool::new(false));
        let inner_is_closed = Arc::clone(&is_closed);

        let connection = Self {
            client_sender: sender,
            server_sender: server_sender.clone(),
            close_sender,
            is_closed,
        };

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
                                        if server_sender.send(Err(ConnectionError::Closing(ClosingReason::ParseError(e))))
                                            .await
                                            .is_err() {
                                            debug!("Could not inform client");
                                        }
                                        inner_close_sender.send(()).await.unwrap();
                                        closing = true;
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {}
                            Err(e) => {
                                debug!("Connection error, closing {:?}", e);
                                if server_sender.send(Err(ConnectionError::Closing(ClosingReason::ConnectionError(e))))
                                    .await
                                .is_err() {
                                    debug!("Could not inform client");
                                }
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

                        inner_is_closed.swap(true, Ordering::Relaxed);
                    }
                };
            }
        });

        connection
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }
    pub async fn emit<T: Into<Frame<ClientCommand>>>(
        &self,
        frame: T,
    ) -> Result<(), SendError<StompMessage<ClientCommand>>> {
        self.client_sender
            .send(StompMessage::Frame(frame.into()))
            .await
    }

    pub async fn heartbeat(&self) -> Result<(), SendError<StompMessage<ClientCommand>>> {
        self.client_sender.send(StompMessage::Ping).await
    }

    pub async fn close(&self) {
        if self
            .server_sender
            .send(Err(ConnectionError::Closing(ClosingReason::Shutdown)))
            .await
            .is_err()
        {
            debug!("Could not inform client");
        }

        self.close_sender.send(()).await;
    }
}
