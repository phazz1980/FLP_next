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

    /// How long a write may stall (e.g. while the board reboots after the port opens).
    /// After that the frame is dropped: the session repeats requests and reports a
    /// silent board itself, so a stalled driver is not a lost link.
    const WRITE_DEADLINE: Duration = Duration::from_millis(500);

    /// Windows USB-serial drivers report a timeout as ERROR_SEM_TIMEOUT (121).
    fn is_timeout(e: &io::Error) -> bool {
        e.kind() == io::ErrorKind::TimedOut || e.raw_os_error() == Some(121)
    }

    impl Transport for SerialTransport {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.read(buf) {
                Ok(n) => Ok(n),
                Err(e) if is_timeout(&e) => Ok(0),
                Err(e) => Err(e),
            }
        }

        fn write_all(&mut self, mut data: &[u8]) -> io::Result<()> {
            let deadline = std::time::Instant::now() + WRITE_DEADLINE;
            while !data.is_empty() {
                match self.0.write(data) {
                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                    Ok(n) => data = &data[n..],
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) if is_timeout(&e) => {
                        if std::time::Instant::now() >= deadline {
                            return Ok(());
                        }
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
    }

    /// Connector for a serial port such as `COM3` at the debug link speed.
    pub fn serial(port: &str) -> Connector {
        serial_at(port, BAUD)
    }

    /// Connector for a serial port at any speed (the port monitor).
    pub fn serial_at(port: &str, baud: u32) -> Connector {
        let port = port.to_owned();
        Box::new(move || {
            let mut p = serialport::new(&port, baud)
                .timeout(Duration::from_millis(50))
                .open()
                .map_err(io::Error::other)?;
            // On DevKit boards DTR/RTS drive EN and GPIO0 through the auto-reset
            // circuit; a half-asserted pair holds the chip in reset or the bootloader.
            // Release both, as the Arduino serial monitor does.
            p.write_data_terminal_ready(false)
                .map_err(io::Error::other)?;
            p.write_request_to_send(false).map_err(io::Error::other)?;
            Ok(Box::new(SerialTransport(p)) as Box<dyn Transport>)
        })
    }
}

#[cfg(feature = "serial")]
pub use serial::{serial, serial_at};
