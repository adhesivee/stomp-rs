use crate::client::interceptor::{ForwardChannel, Forwarder, InterceptorMessage};
use crate::client::ReceiptId;
use crate::protocol::{ClientCommand, Frame};
use log::debug;
use std::collections::HashMap;
use tokio::sync::mpsc::{channel, Sender};

pub async fn receipt_actor() -> Sender<(Forwarder, InterceptorMessage)> {
    let (sender, mut receiver): ForwardChannel = channel(1);

    tokio::spawn(async move {
        let mut pending_receipts: HashMap<ReceiptId, (Forwarder, Frame<ClientCommand>)> =
            HashMap::new();

        loop {
            match receiver.recv().await {
                Some((forwarder, InterceptorMessage::AfterClientSend(frame))) => {
                    if let Some(receipt) = frame.headers.get("receipt") {
                        debug!("Receipt received");
                        pending_receipts.insert(receipt.clone(), (forwarder, frame));
                    } else {
                        forwarder
                            .proceed(InterceptorMessage::AfterClientSend(frame))
                            .await;
                    }
                }
                Some((forwarder, InterceptorMessage::BeforeServerReceive(frame))) => {
                    if let Some(receipt_id) = frame.headers.get("receipt-id") {
                        if let Some(pender) = pending_receipts.remove(receipt_id) {
                            pender
                                .0
                                .proceed(InterceptorMessage::AfterClientSend(pender.1))
                                .await;
                        }
                    }

                    forwarder
                        .proceed(InterceptorMessage::BeforeServerReceive(frame))
                        .await;
                }
                Some((forwarder, message)) => {
                    forwarder.proceed(message).await;
                }
                _ => {}
            }
        }
    });

    sender
}
