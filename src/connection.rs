use crate::protocol::{FrameParser, ServerCommand, StompMessage, ClientCommand, Frame};
use tokio::sync::mpsc::{Sender, channel};
use tokio::net::TcpStream;
use crate::protocol::frame::Connect;
use tokio::sync::mpsc::error::SendError;
use tokio::io::{ErrorKind, AsyncWriteExt};
use crate::protocol::BNF_LF;
use tokio::sync::broadcast::Sender as BroadcastSender;

pub struct Connection {
    client_sender: Sender<StompMessage<ClientCommand>>,
    server_sender: BroadcastSender<StompMessage<ServerCommand>>,
}

impl Connection {
    pub async fn new(
        mut tcp_stream: TcpStream,
        server_sender: BroadcastSender<StompMessage<ServerCommand>>,
    ) -> Self {
        let (sender, mut receiver) = channel(5);

        let inner_sender = server_sender.clone();
        tokio::spawn(async move {
            let mut msg = vec![0; 1024];
            let mut parser: FrameParser<ServerCommand> = FrameParser::new();

            loop {
                tokio::select! {
                    frame = receiver.recv() => {
                         if let Some(message) = frame {
                            match message {
                                StompMessage::Frame(frame) => tcp_stream.write_all(&frame.to_bytes()).await.unwrap(),
                                StompMessage::Ping => tcp_stream.write_u8(BNF_LF).await.unwrap()
                            }

                            tcp_stream.flush();
                        }

                    },
                    _ = tcp_stream.readable() => {
                        match tcp_stream.try_read(&mut msg) {
                            Ok(n) => {
                                if let Ok(messages) = parser.parse(&msg[..n]) {
                                    for message in messages {
                                        inner_sender.send(message).unwrap();
                                    }
                                }

                            }
                            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {

                            }
                            Err(e) => {
                                // @TODO
                                // return Err(e.into());
                            }
                        }
                    }

                }
                ;
            }
        });

        Connection {
            client_sender: sender,
            server_sender,
        }
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
}