use crate::protocol::{ClientCommand, Frame, ServerCommand};
use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait Interceptor {
    async fn before_emit(
        &self,
        frame: Frame<ClientCommand>,
    ) -> Result<Frame<ClientCommand>, Box<dyn Error>>;

    async fn after_emit(&self, frame: &Frame<ClientCommand>) -> Result<(), Box<dyn Error>>;

    async fn before_dispatch(
        &self,
        frame: Frame<ServerCommand>,
    ) -> Result<Frame<ServerCommand>, Box<dyn Error>>;

    async fn after_dispatch(&self, frame: &Frame<ServerCommand>) -> Result<(), Box<dyn Error>>;
}
