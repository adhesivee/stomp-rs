use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use crate::client::{ReceiptId, ServerStompSender};
use std::error::Error;
use crate::protocol::{Frame, ServerCommand, StompMessage};
use tokio::sync::mpsc::error::SendError;

pub(crate) struct ReceiptAwaiter {
    pending_receipts: Arc<Mutex<HashMap<ReceiptId, ServerStompSender>>>,
}

impl ReceiptAwaiter {
    pub(crate) fn new() -> Self {
        Self {
            pending_receipts: Arc::new(Default::default())
        }
    }

    pub(crate) async fn receipt_request(&self, receipt_id: ReceiptId, sender: ServerStompSender) -> Result<(), Box<dyn Error>> {
        let mut guard = self.pending_receipts.lock().await;
        guard.insert(receipt_id, sender);

        Ok(())
    }

    pub(crate) async fn process(&self, frame: &Frame<ServerCommand>) -> Result<(), SendError<StompMessage<ServerCommand>>> {
        if let Some(receipt_id) = frame.headers.get("receipt-id") {
            let mut lock = self.pending_receipts.lock().await;
            if let Some(pending_sender) = lock.remove(receipt_id) {
                pending_sender.send(StompMessage::Frame(frame.clone())).await?;
            };
        }

        Ok(())
    }
}