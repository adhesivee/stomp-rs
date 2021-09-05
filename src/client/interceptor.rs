use crate::protocol::{ClientCommand, Frame, ServerCommand};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::mpsc::{Receiver, Sender};

use tokio::sync::oneshot::channel as OneshotChannel;
use tokio::sync::oneshot::Receiver as OneshotReceiver;

pub type ForwardChannel = (
    Sender<(Forwarder, InterceptorMessage)>,
    Receiver<(Forwarder, InterceptorMessage)>,
);
pub struct Forwarder(
    Vec<Sender<(Forwarder, InterceptorMessage)>>,
    tokio::sync::oneshot::Sender<InterceptorMessage>,
);

impl Forwarder {
    pub fn new(
        forwards: Vec<Sender<(Forwarder, InterceptorMessage)>>,
    ) -> (Self, OneshotReceiver<InterceptorMessage>) {
        let (sender, receiver) = OneshotChannel();
        (Self(forwards, sender), receiver)
    }

    pub async fn proceed(
        mut self,
        message: InterceptorMessage,
    ) -> Result<(), SendError<(Forwarder, InterceptorMessage)>> {
        if let Some(next) = self.0.pop() {
            next.send((self, message)).await?;
        } else {
            self.1.send(message);
        }

        Ok(())
    }
}

pub enum InterceptorMessage {
    BeforeClientSend(Frame<ClientCommand>),
    AfterClientSend(Frame<ClientCommand>),

    BeforeServerReceive(Frame<ServerCommand>),
    AfterServerReceive(Frame<ServerCommand>),
}
