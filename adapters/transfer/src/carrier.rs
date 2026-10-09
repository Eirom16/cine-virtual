//! Byte streams only; no room authority, readiness or route policy.
use std::{
    io::{Read, Write},
    net::TcpStream,
    time::Duration,
};

/// Implementations must bound blocking I/O and preserve backpressure.
/// The owner retains the underlying socket for shutdown on cancellation.
pub trait Carrier: Read + Write {
    fn configure(&self, timeout: Duration) -> std::io::Result<()>;
    /// Flush encrypted termination before an owning socket is dropped.
    fn close(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Carrier for TcpStream {
    fn configure(&self, timeout: Duration) -> std::io::Result<()> {
        self.set_nonblocking(false)?;
        self.set_nodelay(true)?;
        self.set_read_timeout(Some(timeout))?;
        self.set_write_timeout(Some(timeout))
    }
}

// An outer TLS connection is a byte pipe for the *separate* peer TLS session.
impl<S: Carrier> Carrier for rustls::StreamOwned<rustls::ClientConnection, S> {
    fn configure(&self, timeout: Duration) -> std::io::Result<()> {
        self.sock.configure(timeout)
    }
    fn close(&mut self) -> std::io::Result<()> {
        self.conn.send_close_notify();
        self.flush()?;
        self.sock.close()
    }
}
