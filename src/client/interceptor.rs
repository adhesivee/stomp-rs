use crate::protocol::{ClientCommand, Frame, ServerCommand};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};

use tokio::sync::oneshot::channel as OneshotChannel;
use tokio::sync::oneshot::Receiver as OneshotReceiver;
use async_trait::async_trait;

#[async_trait]
pub trait ConnectionHook {
    async fn before_send(&self, frame: &Frame<ClientCommand>);
    async fn after_send(&self, frame: &Frame<ClientCommand>);

    async fn before_receive(&self, frame: &Frame<ServerCommand>);
    async fn after_receive(&self, frame: &Frame<ServerCommand>);
}