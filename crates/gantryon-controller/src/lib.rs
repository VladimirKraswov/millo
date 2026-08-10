use gantryon_domain::{ConnectionState, ControllerSnapshot};
use gantryon_grbl::{StatusParseError, parse_status_line};
use gantryon_transport::{Transport, TransportError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Status(#[from] StatusParseError),
}

pub struct Controller<T> {
    transport: T,
    snapshot: ControllerSnapshot,
}

impl<T: Transport> Controller<T> {
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            snapshot: ControllerSnapshot::default(),
        }
    }

    pub fn snapshot(&self) -> ControllerSnapshot {
        self.snapshot.clone()
    }

    pub async fn connect(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        self.snapshot.connection = ConnectionState::Connecting;
        self.snapshot.last_error = None;

        if let Err(error) = self.transport.connect().await {
            self.snapshot.connection = ConnectionState::Faulted;
            self.snapshot.last_error = Some(error.to_string());
            return Err(error.into());
        }

        self.snapshot.connection = ConnectionState::Connected;
        Ok(self.snapshot())
    }

    pub async fn disconnect(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        if let Err(error) = self.transport.disconnect().await {
            self.snapshot.connection = ConnectionState::Faulted;
            self.snapshot.last_error = Some(error.to_string());
            return Err(error.into());
        }

        self.snapshot.connection = ConnectionState::Disconnected;
        self.snapshot.last_error = None;
        Ok(self.snapshot())
    }

    pub async fn refresh_status(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        let result = self.request_status().await;
        match result {
            Ok(()) => {
                self.snapshot.last_error = None;
                Ok(self.snapshot())
            }
            Err(error) => {
                if !self.transport.is_connected() {
                    self.snapshot.connection = ConnectionState::Faulted;
                }
                self.snapshot.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    async fn request_status(&mut self) -> Result<(), ControllerError> {
        self.transport.write(b"?").await?;
        let line = self.transport.read_line().await?;
        self.snapshot.machine = parse_status_line(&line)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use gantryon_domain::{ConnectionState, MachineMode};
    use gantryon_mock::MockTransport;

    use super::*;

    #[tokio::test]
    async fn completes_the_mock_status_round_trip() {
        let transport = MockTransport::with_status(
            "<Run|MPos:10.000,20.000,-1.500|WPos:1.000,2.000,-1.500|FS:300,8000>",
        );
        let mut controller = Controller::new(transport);

        controller.connect().await.unwrap();
        let snapshot = controller.refresh_status().await.unwrap();

        assert_eq!(snapshot.connection, ConnectionState::Connected);
        assert_eq!(snapshot.machine.mode, MachineMode::Run);
        assert_eq!(snapshot.machine.machine_position.unwrap().x, 10.0);
        assert_eq!(snapshot.machine.feed_rate, 300.0);
        assert_eq!(snapshot.machine.spindle_speed, 8000.0);
    }

    #[tokio::test]
    async fn reports_status_requests_before_connect() {
        let mut controller = Controller::new(MockTransport::default());

        let error = controller.refresh_status().await.unwrap_err();

        assert!(matches!(
            error,
            ControllerError::Transport(TransportError::NotConnected)
        ));
        assert_eq!(controller.snapshot().connection, ConnectionState::Faulted);
    }
}
