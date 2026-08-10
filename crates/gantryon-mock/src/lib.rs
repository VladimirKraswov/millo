use std::collections::VecDeque;

use async_trait::async_trait;
use gantryon_transport::{Transport, TransportError};

const DEFAULT_STATUS: &str = "<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>";

#[derive(Debug)]
pub struct MockTransport {
    connected: bool,
    status_line: String,
    responses: VecDeque<String>,
    writes: Vec<Vec<u8>>,
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::with_status(DEFAULT_STATUS)
    }
}

impl MockTransport {
    pub fn with_status(status_line: impl Into<String>) -> Self {
        Self {
            connected: false,
            status_line: status_line.into(),
            responses: VecDeque::new(),
            writes: Vec::new(),
        }
    }

    pub fn writes(&self) -> &[Vec<u8>] {
        &self.writes
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.responses.clear();
        Ok(())
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        self.writes.push(data.to_vec());
        if data == b"?" {
            self.responses.push_back(self.status_line.clone());
        }
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }

        self.responses.pop_front().ok_or(TransportError::NoData)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn responds_to_realtime_status_query() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();

        let response = transport.read_line().await.unwrap();

        assert_eq!(response, DEFAULT_STATUS);
        assert_eq!(transport.writes(), &[b"?".to_vec()]);
    }
}
