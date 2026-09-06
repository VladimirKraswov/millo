#![cfg_attr(not(unix), allow(dead_code))]

#[cfg(unix)]
mod unix {
    use std::{
        ffi::CStr,
        io,
        mem::MaybeUninit,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };

    use millo_mock::MockTransport;
    use millo_serial::{
        ExternalSerialRegistration, SerialPortDescriptor, SerialPortKind,
        register_external_serial_endpoint,
    };
    use millo_transport::{Transport, TransportError};
    use tokio::{io::unix::AsyncFd, sync::Mutex, task::JoinHandle, time::timeout};

    const PRODUCT: &str = "Millo VMC-3 GRBL Controller";
    const SERIAL: &str = "MILLO-VMC3-0001";
    const MAX_INPUT_LINE: usize = 4 * 1024;

    pub struct VirtualController {
        port_name: PathBuf,
        task: JoinHandle<io::Result<()>>,
        _slave: OwnedFd,
        _registration: ExternalSerialRegistration,
    }

    impl VirtualController {
        pub async fn start() -> io::Result<Self> {
            Self::start_configured(false).await
        }

        /// Starts the explicit angular XYZA mock firmware on a new virtual PTY.
        pub async fn start_rotary() -> io::Result<Self> {
            Self::start_configured(true).await
        }

        async fn start_configured(rotary: bool) -> io::Result<Self> {
            let (master, slave, port_name) = open_raw_pty()?;
            let descriptor = SerialPortDescriptor {
                port_name: port_name.to_string_lossy().into_owned(),
                kind: SerialPortKind::Unknown,
                vendor_id: None,
                product_id: None,
                manufacturer: Some("Millo".to_owned()),
                product: Some(
                    if rotary {
                        "Millo VMC-4 XYZA Controller"
                    } else {
                        PRODUCT
                    }
                    .to_owned(),
                ),
                serial_number: Some(if rotary { "MILLO-VMC4-0001" } else { SERIAL }.to_owned()),
            };
            let registration =
                register_external_serial_endpoint(&descriptor).map_err(transport_io_error)?;
            let task = tokio::spawn(serve(master, rotary));
            Ok(Self {
                port_name,
                task,
                _slave: slave,
                _registration: registration,
            })
        }

        pub fn port_name(&self) -> &Path {
            &self.port_name
        }

        pub async fn wait(self) -> io::Result<()> {
            self.task
                .await
                .map_err(|error| io::Error::other(error.to_string()))?
        }
    }

    fn open_raw_pty() -> io::Result<(OwnedFd, OwnedFd, PathBuf)> {
        let mut master = -1;
        let mut slave = -1;
        let result = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        let master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        configure_raw(&slave)?;
        set_nonblocking(&master)?;
        let mut name = [0_i8; 256];
        let result = unsafe { libc::ttyname_r(slave.as_raw_fd(), name.as_mut_ptr(), name.len()) };
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        let path = unsafe { CStr::from_ptr(name.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        Ok((master, slave, PathBuf::from(path)))
    }

    fn configure_raw(fd: &OwnedFd) -> io::Result<()> {
        let mut termios = MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(fd.as_raw_fd(), termios.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut termios = unsafe { termios.assume_init() };
        unsafe { libc::cfmakeraw(&mut termios) };
        if unsafe { libc::tcsetattr(fd.as_raw_fd(), libc::TCSANOW, &termios) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn set_nonblocking(fd: &OwnedFd) -> io::Result<()> {
        let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    async fn serve(master: OwnedFd, rotary: bool) -> io::Result<()> {
        let io = Arc::new(AsyncFd::new(master)?);
        let firmware = Arc::new(Mutex::new(if rotary {
            MockTransport::rotary()
        } else {
            MockTransport::default()
        }));
        firmware
            .lock()
            .await
            .connect()
            .await
            .map_err(transport_io_error)?;
        let mut input = Vec::with_capacity(128);
        loop {
            let byte = read_byte(&io).await?;
            if is_realtime(byte) {
                if byte == 0x18 {
                    input.clear();
                }
                dispatch(&io, &firmware, &[byte]).await?;
                continue;
            }
            if byte == b'\r' {
                continue;
            }
            input.push(byte);
            if input.len() > MAX_INPUT_LINE {
                input.clear();
                write_line(&io, "error:11").await?;
                continue;
            }
            if byte == b'\n' {
                dispatch(&io, &firmware, &input).await?;
                input.clear();
            }
        }
    }

    async fn dispatch(
        io: &Arc<AsyncFd<OwnedFd>>,
        firmware: &Arc<Mutex<MockTransport>>,
        data: &[u8],
    ) -> io::Result<()> {
        let mut firmware = firmware.lock().await;
        firmware.write(data).await.map_err(transport_io_error)?;
        loop {
            match timeout(Duration::from_millis(2), firmware.read_line()).await {
                Ok(Ok(line)) => write_line(io, &line).await?,
                Ok(Err(TransportError::NoData)) | Err(_) => break,
                Ok(Err(error)) => return Err(transport_io_error(error)),
            }
        }
        Ok(())
    }

    async fn read_byte(io: &AsyncFd<OwnedFd>) -> io::Result<u8> {
        loop {
            let mut ready = io.readable().await?;
            let result = ready.try_io(|inner| {
                let mut byte = 0_u8;
                let read = unsafe {
                    libc::read(
                        inner.get_ref().as_raw_fd(),
                        (&mut byte as *mut u8).cast(),
                        1,
                    )
                };
                if read == 1 {
                    Ok(byte)
                } else if read == 0 {
                    Err(io::Error::new(io::ErrorKind::UnexpectedEof, "PTY closed"))
                } else {
                    Err(io::Error::last_os_error())
                }
            });
            match result {
                Ok(result) => return result,
                Err(_) => continue,
            }
        }
    }

    async fn write_line(io: &AsyncFd<OwnedFd>, line: &str) -> io::Result<()> {
        let mut payload = line.as_bytes().to_vec();
        payload.extend_from_slice(b"\r\n");
        let mut offset = 0;
        while offset < payload.len() {
            let mut ready = io.writable().await?;
            let result = ready.try_io(|inner| {
                let written = unsafe {
                    libc::write(
                        inner.get_ref().as_raw_fd(),
                        payload[offset..].as_ptr().cast(),
                        payload.len() - offset,
                    )
                };
                if written < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(written as usize)
                }
            });
            match result {
                Ok(Ok(written)) => offset += written,
                Ok(Err(error)) => return Err(error),
                Err(_) => continue,
            }
        }
        Ok(())
    }

    fn is_realtime(byte: u8) -> bool {
        matches!(byte, b'?' | b'!' | b'~' | 0x18 | 0x85) || byte >= 0x80
    }

    fn transport_io_error(error: impl std::fmt::Display) -> io::Error {
        io::Error::other(error.to_string())
    }
}

#[cfg(unix)]
pub use unix::VirtualController;

#[cfg(not(unix))]
compile_error!("millo-virtual-controller currently requires a Unix PTY host");
