use crate::client::interceptor::{ConnectionHook};
use crate::client::ReceiptId;
use crate::protocol::{ClientCommand, Frame, ServerCommand};
use log::debug;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneshotSender, Receiver as OneshotReceiver};
use async_trait::async_trait;
use tokio::time::Duration;

pub struct ReceiptHandler {
    pending_receipts: Arc<Mutex<HashMap<ReceiptId, (Option<OneshotReceiver<()>>, OneshotSender<()>)>>>,
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
            let (tx, rx) = oneshot_channel();

            {
                self.pending_receipts.lock()
                    .unwrap()
                    .insert(receipt.clone(), (Some(rx), tx));
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
                    .get_mut(receipt)
                    .unwrap()
                    .0
                    .take()
                    .unwrap()
            };

            if let Err(_) = tokio::time::timeout(Duration::from_secs(30), rx).await {
                self.pending_receipts
                    .lock()
                    .unwrap()
                    .remove(receipt);
            }
        }
    }

    async fn before_receive(&self, frame: &Frame<ServerCommand>) {
        if let Some(receipt_id) = frame.headers.get("receipt-id") {
            let pender = {
                self.pending_receipts.lock()
                    .unwrap()
                    .remove(receipt_id)
            };
            if let Some(pender) = pender {
                pender.1.send(()).unwrap();
            }
        }
    }
}