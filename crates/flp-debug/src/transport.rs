//! Transport layer: USB serial now, Wi-Fi later behind the same trait.

use std::io;

/// Byte stream to the board. `read` returns 0 on timeout (no data yet).
pub trait Transport: Send {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;
}

/// Opens (or reopens after a drop) the connection.
pub type Connector = Box<dyn Fn() -> io::Result<Box<dyn Transport>> + Send>;

/// Debug link speed; must match `Serial.begin` in the firmware module.
pub const BAUD: u32 = 115_200;

#[cfg(feature = "serial")]
mod serial {
    use std::io::{self, Read, Write};
    use std::time::Duration;

    use super::{Connector, Transport, BAUD};

    struct SerialTransport(Box<dyn serialport::SerialPort>);

    impl Transport for SerialTransport {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.read(buf) {
                Ok(n) => Ok(n),
                Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(0),
                Err(e) => Err(e),
            }
        }

        fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
            Write::write_all(&mut self.0, data)
        }
    }

    /// Connector for a serial port such as `COM3`.
    pub fn serial(port: &str) -> Connector {
        let port = port.to_owned();
        Box::new(move || {
            let p = serialport::new(&port, BAUD)
                .timeout(Duration::from_millis(50))
                .open()
                .map_err(io::Error::other)?;
            Ok(Box::new(SerialTransport(p)) as Box<dyn Transport>)
        })
    }
}

#[cfg(feature = "serial")]
pub use serial::serial;
