use async_trait::async_trait;
use std::error::Error;
use crate::protocol::{Frame, ClientCommand, ServerCommand};

#[async_trait]
pub trait Interceptor {
    async fn before_emit(&self, frame: Frame<ClientCommand>) -> Result<Frame<ClientCommand>, Box<dyn Error>> { Ok(frame) }

    async fn after_emit(&self, frame: &Frame<ClientCommand>) -> Result<(), Box<dyn Error>> { Ok(()) }

    async fn before_dispatch(&self, frame: Frame<ServerCommand>) -> Result<Frame<ServerCommand>, Box<dyn Error>> { Ok(frame) }

    async fn after_dispatch(&self, frame: &Frame<ServerCommand>) -> Result<(), Box<dyn Error>> { Ok(()) }
}