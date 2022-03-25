use crate::client::handler::receipt_awaiter::ReceiptHandler;
use crate::client::handler::subscribers::{SubscriberHandler};
use crate::client::interceptor::{ConnectionHook};
use crate::client::{ClientBuilder, ClientError};
use crate::connection::{Connection, ConnectionError};
use crate::protocol::frame::{Ack, Connect, Nack, Send, Subscribe};
use crate::protocol::{ClientCommand, Frame, ServerCommand, StompMessage};
use log::debug;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) struct InternalClient {
    connection: Arc<Connection>,
    subscriber: SubscriberHandler,
    hooks: Arc<Vec<Box<dyn ConnectionHook>>>,
}

impl InternalClient {
    pub(crate) async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let (sender, receiver) = channel(5);

        let connection = Arc::new(
            Connection::new(TcpStream::connect(builder.host.clone()).await?, sender).await,
        );

        let subscriber = SubscriberHandler::new().await;

        let client = Self {
            connection,
            subscriber: subscriber.clone(),
            hooks: Arc::new(vec![
                Box::new(ReceiptHandler::new()),
                Box::new(subscriber),
            ]),
        };

        let server_timeout: u128 = builder.heartbeat.unwrap_or((0, 0)).1.into();

        let (connected_sender, connected_receiver) = tokio::sync::oneshot::channel();

        client
            .spawn_server_frame_listener(connected_sender, receiver, server_timeout)
            .await;

        let mut connect_frame = Connect::new("1.2".to_owned(), builder.host);

        if let Some(heartbeat) = builder.heartbeat {
            connect_frame = connect_frame.heartbeat(heartbeat.0, heartbeat.1);
        }

        client.emit(connect_frame.into()).await?;
        let first_frame = connected_receiver.await?;

        if let ServerCommand::Connected = first_frame.command {
            Ok(client)
        } else {
            // @TODO: Include close reason
            client.connection.close().await;

            Err(Box::new(ClientError::ConnectionError(None)))
        }
    }

    async fn spawn_server_frame_listener(
        &self,
        connected_sender: tokio::sync::oneshot::Sender<Frame<ServerCommand>>,
        mut receiver: Receiver<Result<StompMessage<ServerCommand>, ConnectionError>>,
        server_timeout: u128,
    ) {
        let connection = Arc::clone(&self.connection.clone());
        let hooks = Arc::clone(&self.hooks);

        tokio::spawn(async move {
            let mut last_heartbeat = Instant::now();

            let mut connected_sender = Some(connected_sender);

            loop {
                if connection.is_closed().await {
                    debug!("Connection closed, closing client");
                    receiver.close();
                }

                if server_timeout > 0 && last_heartbeat.elapsed().as_millis() > server_timeout {
                    connection.clone().close().await;
                }

                if let Ok(message) =
                    tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await
                {
                    match message {
                        Some(Ok(message)) => match message {
                            StompMessage::Frame(frame) => {
                                debug!("Frame received: {:?}", frame.clone());
                                last_heartbeat = Instant::now();

                                for hook in hooks.iter() {
                                    hook.before_receive(&frame).await;
                                }

                                if !connected_sender
                                    .as_ref()
                                    .map(|val| val.is_closed())
                                    .unwrap_or_else(|| true)
                                {
                                    connected_sender
                                        .take()
                                        .unwrap()
                                        .send(frame.clone())
                                        .unwrap();
                                }


                                for hook in hooks.iter() {
                                    hook.after_receive(&frame).await;
                                }
                            }
                            StompMessage::Ping => last_heartbeat = Instant::now(),
                        },
                        Some(Err(_)) => {
                            break;
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        });
    }

    pub(crate) async fn subscribe(
        &self,
        subscribe: Subscribe,
        sender: Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        let destination = subscribe.headers["destination"].clone();
        let subscriber_id = subscribe.headers["id"].clone();
        let receipt_id = Uuid::new_v4();

        self.emit(subscribe.receipt(receipt_id.to_string()).into())
            .await?;

        self.subscriber
            .register(subscriber_id, destination, sender)
            .await;

        Ok(())
    }

    pub(crate) async fn send(&self, send: Send) -> Result<(), Box<dyn Error>> {
        let receipt_id = Uuid::new_v4();

        self.emit(send.receipt(receipt_id.to_string()).into())
            .await?;

        Ok(())
    }

    pub(crate) async fn emit(&self, frame: Frame<ClientCommand>) -> Result<(), Box<dyn Error>> {
        debug!("Emit frame");

        for hook in self.hooks.iter() {
            hook.before_send(&frame).await;
        }

        self.connection.emit(frame.clone()).await?;

        for hook in self.hooks.iter() {
            hook.after_send(&frame).await;
        }

        Ok(())
    }

    pub(crate) async fn ack(&self, ack: Ack) -> Result<(), Box<dyn Error>> {
        self.emit(ack.receipt(Uuid::new_v4().to_string()).into())
            .await
    }

    pub(crate) async fn nack(&self, nack: Nack) -> Result<(), Box<dyn Error>> {
        self.emit(nack.receipt(Uuid::new_v4().to_string()).into())
            .await
    }
}
