use crate::client::interceptor::{ConnectionHook};
use crate::client::ReceiptId;
use crate::protocol::{ClientCommand, Frame, ServerCommand};
use log::debug;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneshotSender, Receiver as OneshotReceiver};
use async_trait::async_trait;
use tokio::sync::Notify;
use tokio::time::Duration;

pub struct ReceiptHandler {
    pending_receipts: Arc<Mutex<HashMap<ReceiptId, Arc<Notify>>>>,
}

impl ReceiptHandler {
    pub fn new() -> Self {
        Self {
            pending_receipts: Arc::new(Mutex::new(Default::default()))
        }
    }
}

#[async_trait]
impl ConnectionHook for ReceiptHandler {
    async fn before_send(&self, frame: &Frame<ClientCommand>) {
        if let Some(receipt) = frame.headers.get("receipt") {
            {
                self.pending_receipts.lock()
                    .unwrap()
                    .insert(receipt.clone(), Arc::new(Notify::new()));
            }
        }
    }

    async fn after_send(&self, frame: &Frame<ClientCommand>) {
        if let Some(receipt) = frame.headers.get("receipt") {
            // @TODO: Receipt cleanup
            debug!("Receipt received");
            let rx = {
                self.pending_receipts
                    .lock()
                    .unwrap()
                    .get(receipt)
                    .map(|notify| Arc::clone(notify))
            };

            if let Some(rx) = rx {
                if let Err(_) = tokio::time::timeout(Duration::from_secs(30), rx.notified()).await {
                    self.pending_receipts
                        .lock()
                        .unwrap()
                        .remove(receipt);
                }
            }
        }
    }

    async fn before_receive(&self, frame: &Frame<ServerCommand>) {
        if let ServerCommand::Receipt = frame.command {
            if let Some(receipt_id) = frame.headers.get("receipt-id") {
                let pender = {
                    self.pending_receipts.lock()
                        .unwrap()
                        .remove(receipt_id)
                };
                if let Some(pender) = pender {
                    pender.notify_one();
                }
            }
        }
    }
}