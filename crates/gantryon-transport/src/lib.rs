use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("transport is not connected")]
    NotConnected,
    #[error("transport has no data available")]
    NoData,
    #[error("transport I/O failed: {0}")]
    Io(String),
}

#[async_trait]
pub trait Transport: Send {
    async fn connect(&mut self) -> Result<(), TransportError>;
    async fn disconnect(&mut self) -> Result<(), TransportError>;
    async fn write(&mut self, data: &[u8]) -> Result<(), TransportError>;
    async fn read_line(&mut self) -> Result<String, TransportError>;
    fn is_connected(&self) -> bool;
}
