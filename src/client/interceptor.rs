use crate::protocol::{ClientCommand, Frame, ServerCommand};
use async_trait::async_trait;

#[async_trait]
pub trait ConnectionHook: Send + Sync {
    async fn before_send(&self, _frame: &Frame<ClientCommand>) { }
    async fn after_send(&self, _frame: &Frame<ClientCommand>) { }

    async fn before_receive(&self, _frame: &Frame<ServerCommand>) { }
    async fn after_receive(&self, _frame: &Frame<ServerCommand>) { }
}