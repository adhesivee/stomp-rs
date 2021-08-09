use crate::protocol::{FrameParser, ServerCommand, StompMessage, ClientCommand, Frame};
use tokio::sync::mpsc::{Sender, channel};
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::SendError;
use tokio::io::{ErrorKind, AsyncWriteExt, AsyncReadExt};
use crate::protocol::BNF_LF;
use tokio::sync::oneshot::Receiver;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;

pub struct Connection {
    client_sender: Sender<StompMessage<ClientCommand>>,
    server_sender: Sender<StompMessage<ServerCommand>>,
    close_sender: Sender<()>,
    is_closed: Arc<Mutex<bool>>,
}

impl Connection {
    pub async fn new(
        mut tcp_stream: TcpStream,
        server_sender: Sender<StompMessage<ServerCommand>>,
    ) -> Self {
        let (sender, mut receiver) = channel(5);

        let inner_sender = server_sender.clone();
        let (close_sender, mut close_receiver) = channel(1);
        let inner_close_sender = close_sender.clone();
        let is_closed = Arc::new(Mutex::new(false));
        let inner_is_closed = Arc::clone(&is_closed);

        tokio::spawn(async move {
            let mut msg = vec![0; 8096];
            let mut parser: FrameParser<ServerCommand> = FrameParser::new();

            loop {
                tokio::select! {
                    frame = receiver.recv() => {
                         if let Some(message) = frame {
                            match message {
                                StompMessage::Frame(frame) => tcp_stream.write_all(&frame.to_bytes()).await.unwrap(),
                                StompMessage::Ping => tcp_stream.write_u8(BNF_LF).await.unwrap()
                            }

                            tcp_stream.flush().await.unwrap();
                        }
                    },
                    read = tcp_stream.read(&mut msg) => {
                        match read {
                            Ok(n) => {
                                match parser.parse(&msg[..n]) {
                                    Ok(messages) => {
                                        for message in messages {
                                            inner_sender.send(message).await.unwrap();
                                        }
                                    }
                                    Err(_) => {
                                        // @TODO: Report cause
                                        inner_close_sender.send(()).await.unwrap();
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {

                            }
                            Err(_e) => {
                                // @TODO: report cause
                                inner_close_sender.send(()).await;
                            }
                        }
                    }
                    _ = close_receiver.recv() => {
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
            server_sender,
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
        self.close_sender.send(());
    }
}