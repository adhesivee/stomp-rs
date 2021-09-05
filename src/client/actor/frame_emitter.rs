use crate::client::interceptor::{ForwardChannel, Forwarder, InterceptorMessage};
use crate::connection::Connection;
use log::debug;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Sender};

pub async fn frame_emitter_actor(
    connection: Arc<Connection>,
) -> Sender<(Forwarder, InterceptorMessage)> {
    let (sender, mut receiver): ForwardChannel = channel(1);

    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some((forwarder, InterceptorMessage::BeforeClientSend(frame))) => {
                    debug!("Emit frame by interceptor");
                    connection.emit(frame.clone()).await;
                    forwarder
                        .proceed(InterceptorMessage::BeforeClientSend(frame))
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
