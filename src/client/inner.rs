use uuid::Uuid;
use std::error::Error;
use crate::protocol::{StompMessage, ServerCommand, Frame, ClientCommand};
use crate::protocol::frame::{Unsubscribe, Connect, Subscribe, Send, Ack, Nack};
use tokio::time::{Instant, Duration};
use tokio::sync::mpsc::{channel, Sender, Receiver};
use crate::client::{ClientError, ServerStompSender, SubscriberId, ReceiptId, ClientBuilder};
use std::sync::Arc;
use crate::connection::Connection;
use tokio::sync::Mutex;
use std::collections::HashMap;
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::SendError;
use std::mem::{swap, take};

pub(crate) struct InnerClient {
    pub(crate) connection: Arc<Connection>,
    sender: ServerStompSender,
    pending_receipts: Arc<Mutex<HashMap<ReceiptId, ServerStompSender>>>,
    subscribers: Arc<Mutex<HashMap<SubscriberId, ServerStompSender>>>,
}

impl InnerClient {
    pub(crate) async fn connect(builder: ClientBuilder) -> Result<Self, Box<dyn Error>> {
        let (sender, mut receiver) = channel(5);

        let client = Self {
            connection: Arc::new(Connection::new(
                TcpStream::connect(builder.host.clone()).await?,
                sender.clone(),
            ).await),
            sender,
            pending_receipts: Arc::new(Default::default()),
            subscribers: Arc::new(Default::default()),
        };

        let subscribers = Arc::clone(&client.subscribers);
        let pending_receipts = Arc::clone(&client.pending_receipts);
        let server_timeout: u128 = builder.heartbeat.unwrap_or_else(|| (0, 0)).1.into();
        let connection = client.connection.clone();

        let (connected_sender, mut connected_receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut last_heartbeat = Instant::now();

            let mut connected_sender = Some(connected_sender);
            loop {
                if connection.is_closed().await {
                    receiver.close();
                }

                if server_timeout > 0 && last_heartbeat.elapsed().as_millis() > server_timeout {
                    connection
                        .clone()
                        .close();
                }

                if let Ok(message) = tokio::time::timeout(Duration::from_millis(100), receiver.recv()).await {
                    match message {
                        Some(StompMessage::Frame(frame)) => {
                            last_heartbeat = Instant::now();

                            if !connected_sender.as_ref().map(|val| val.is_closed()).unwrap_or_else(|| true) {
                                connected_sender.take()
                                    .unwrap()
                                    .send(frame.clone()).unwrap();
                            }

                            if let Some(receipt_id) = frame.headers.get("receipt-id") {
                                let lock = pending_receipts.lock().await;
                                if let Some(pending_sender) = lock.get(receipt_id) {
                                    pending_sender.send(StompMessage::Frame(frame.clone())).await;
                                }
                                drop(lock);
                            }

                            if let Some(subscription) = frame.headers.get("subscription") {
                                let lock = subscribers.lock().await;
                                if let Some(sub_sender) = lock.get(subscription) {
                                    sub_sender.send(StompMessage::Frame(frame.clone())).await;
                                }
                                drop(lock);
                            }
                        }
                        Some(StompMessage::Ping) => {
                            last_heartbeat = Instant::now()
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        });

        let mut connect_frame = Connect::new("1.2".to_owned(), builder.host);

        if let Some(heartbeat) = builder.heartbeat {
            connect_frame = connect_frame.heartbeat(heartbeat.0, heartbeat.1);
        }

        client.emit(connect_frame.into()).await?;
        let first_frame = connected_receiver.await?;

        if let ServerCommand::Connected = first_frame.command{
            Ok(client)
        } else {
            // @TODO: Include close reason
            client.connection.close();

            Err(Box::new(ClientError::ConnectionError(None)))
        }
    }

    pub(crate) async fn subscribe(
        &self,
        destination: String,
        headers: Option<&[(&str, &str)]>,
        sender: Sender<Frame<ServerCommand>>,
    ) -> Result<(), Box<dyn Error>> {
        let subscriber_id = Uuid::new_v4();
        let receipt_id = Uuid::new_v4();

        let subscribe = headers.unwrap_or_else(|| &[])
            .iter()
            .fold(
                Subscribe::new(subscriber_id.to_string(), destination.clone()),
                |subscribe, header| subscribe.header(header.0.to_string(), header.1.to_string()),
            )
            .receipt(receipt_id.to_string());

        self.emit(subscribe.into()).await?;

        let sub_connection = Arc::clone(&self.connection);

        let (sub_sender, receiver) = channel(5);
        let mut lock = self.subscribers.lock().await;
        lock.insert(subscriber_id.to_string(), sub_sender);
        drop(lock);

        tokio::spawn(async move {
            let mut sub_recv = receiver;

            while let Some(message) = sub_recv.recv().await {
                if let StompMessage::Frame(frame) = message {
                    let destination_header = frame.headers.get("destination");

                    if destination_header.is_some() && destination_header.unwrap() == &destination &&
                        sender.send(frame).await.is_err()
                    {
                        break;
                    }
                }
            }

            sub_connection.emit(
                Unsubscribe::new(subscriber_id.to_string())
            ).await.unwrap();
        });
        Ok(())
    }

    pub(crate) async fn send(
        &self,
        destination: String,
        message: String,
        headers: Option<&[(&str, &str)]>,
    ) -> Result<(), Box<dyn Error>> {
        let receipt_id = Uuid::new_v4();

        let send = headers.unwrap_or_else(|| &[])
            .iter()
            .fold(
                Send::new(destination.clone()),
                |send, header| { send.header(header.0.to_string(), header.1.to_string()) },
            )
            .receipt(receipt_id.to_string())
            .body(message);

        self.emit(send.into()).await?;

        Ok(())
    }

    pub(crate) async fn emit(&self, frame: Frame<ClientCommand>) -> Result<(), Box<dyn Error>> {
        let receipt = frame.headers.get("receipt").cloned();

        if let Some(receipt) = receipt {
            let (sender, mut receiver) = channel(1);

            let mut lock = self.pending_receipts.lock().await;
            lock.insert(receipt.clone(), sender);
            drop(lock);

            self.connection.emit(frame).await?;

            self.await_receipt(receipt.clone(), receiver, "".to_string())
                .await?;
        } else {
            self.connection.emit(frame).await?;
        }

        Ok(())
    }

    pub(crate) async fn ack(&self, ack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        self.emit(
            headers.unwrap_or_else(|| &[])
                .iter()
                .fold(
                    Ack::new(ack_id),
                    |ack, header| {
                        ack.header(header.0.to_string(), header.1.to_string())
                    },
                )
                .receipt(Uuid::new_v4().to_string())
                .into()
        ).await
    }

    pub(crate) async fn nack(&self, nack_id: String, headers: Option<&[(&str, &str)]>) -> Result<(), Box<dyn Error>> {
        self.emit(
            headers.unwrap_or_else(|| &[])
                .iter()
                .fold(
                    Nack::new(nack_id),
                    |nack, header| {
                        nack.header(header.0.to_string(), header.1.to_string())
                    },
                )
                .receipt(Uuid::new_v4().to_string())
                .into()
        ).await
    }

    async fn await_receipt(
        &self,
        receipt_id: String,
        mut receipt_receiver: Receiver<StompMessage<ServerCommand>>,
        destination: String,
    ) -> Result<(), Box<dyn Error>> {
        let start = Instant::now();

        loop {
            match tokio::time::timeout(Duration::from_millis(10), receipt_receiver.recv()).await {
                Ok(Some(StompMessage::Frame(val))) => {
                    match val.command {
                        ServerCommand::Receipt => {
                            self.clean_receipt(&receipt_id).await;
                            return Ok(());
                        }
                        ServerCommand::Error => {
                            self.clean_receipt(&receipt_id).await;
                            return Err(Box::new(ClientError::Nack(format!("No received during subscribe of {}", destination))));
                        }
                        _ => { /* non-relevant frame */ }
                    }
                }
                Ok(_) => { /* ignore, message not relevant for this process */ }
                Err(_) => { /* elapsed time check done later */ }
            }

            if start.elapsed().as_millis() > 2000 {
                self.clean_receipt(&receipt_id).await;
                return Err(Box::new(ClientError::ReceiptTimeout("".to_owned())));
            }
        }
    }

    async fn clean_receipt(&self, receipt_id: &String) {
        let mut lock = self.pending_receipts.lock().await;
        lock.remove(receipt_id);
    }
}