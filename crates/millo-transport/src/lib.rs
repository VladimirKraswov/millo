use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("transport is not connected")]
    NotConnected,
    #[error("transport has no data available")]
    NoData,
    #[error("transport line exceeds the {limit}-byte limit")]
    LineTooLong { limit: usize },
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

pub type BoxedTransport = Box<dyn Transport>;

#[derive(Debug, Default)]
pub struct DisconnectedTransport;

#[async_trait]
impl Transport for DisconnectedTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        Err(TransportError::NotConnected)
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        Ok(())
    }

    async fn write(&mut self, _data: &[u8]) -> Result<(), TransportError> {
        Err(TransportError::NotConnected)
    }

    async fn read_line(&mut self) -> Result<String, TransportError> {
        Err(TransportError::NotConnected)
    }

    fn is_connected(&self) -> bool {
        false
    }
}

#[async_trait]
impl<T: Transport + ?Sized> Transport for Box<T> {
    async fn connect(&mut self) -> Result<(), TransportError> {
        (**self).connect().await
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        (**self).disconnect().await
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        (**self).write(data).await
    }

    async fn read_line(&mut self) -> Result<String, TransportError> {
        (**self).read_line().await
    }

    fn is_connected(&self) -> bool {
        (**self).is_connected()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubTransport {
        connected: bool,
        writes: Vec<Vec<u8>>,
    }

    #[async_trait]
    impl Transport for StubTransport {
        async fn connect(&mut self) -> Result<(), TransportError> {
            self.connected = true;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), TransportError> {
            self.connected = false;
            Ok(())
        }

        async fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
            self.writes.push(data.to_vec());
            Ok(())
        }

        async fn read_line(&mut self) -> Result<String, TransportError> {
            Ok("ok".to_owned())
        }

        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    #[tokio::test]
    async fn boxed_transport_preserves_the_transport_contract() {
        let mut transport: BoxedTransport = Box::new(StubTransport {
            connected: false,
            writes: Vec::new(),
        });

        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();

        assert!(transport.is_connected());
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        transport.disconnect().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[tokio::test]
    async fn disconnected_transport_is_a_non_connectable_startup_placeholder() {
        let mut transport = DisconnectedTransport;

        assert_eq!(transport.connect().await, Err(TransportError::NotConnected));
        assert_eq!(
            transport.write(b"?").await,
            Err(TransportError::NotConnected)
        );
        assert_eq!(
            transport.read_line().await,
            Err(TransportError::NotConnected)
        );
        assert!(!transport.is_connected());
        assert_eq!(transport.disconnect().await, Ok(()));
    }
}
