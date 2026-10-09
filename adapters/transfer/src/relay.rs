//! Experimental single-pair relay. No public service, storage or room signaling.
//! Two peers connect outward; outer TLS admits separate single-use tickets.
//! The forwarded payload remains encrypted by the *inner* peer TLS session.
use crate::{Authorization, Control, Credential, Error, carrier::Carrier, transport};
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use serde::Serialize;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};

pub struct BoundedSocket {
    socket: TcpStream,
    until: Option<Instant>,
    control: Control,
}
impl BoundedSocket {
    fn check(&self) -> std::io::Result<()> {
        self.control
            .check()
            .map_err(|_| std::io::ErrorKind::ConnectionAborted)?;
        if let Some(until) = self.until {
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            self.socket
                .configure(remaining.min(Duration::from_secs(3)))?;
        }
        Ok(())
    }
}
impl Carrier for BoundedSocket {
    fn configure(&self, timeout: Duration) -> std::io::Result<()> {
        self.socket.configure(timeout)
    }
}
impl Read for BoundedSocket {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.check()?;
        self.socket.read(bytes)
    }
}
impl Write for BoundedSocket {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.check()?;
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.check()?;
        self.socket.flush()
    }
}

pub type Pipe = StreamOwned<ClientConnection, BoundedSocket>;

/// The ticket is distinct from the peer grant. Never give a relay the inner secret.
/// Callers must retain/shutdown the observed socket on lifecycle changes.
pub fn connect(
    address: SocketAddr,
    certificate: &[u8],
    ticket: &Credential,
    control: &Control,
    mut observed: impl FnMut(&TcpStream),
) -> Result<Pipe, Error> {
    control.check()?;
    let socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    socket.configure(Duration::from_secs(3))?;
    observed(&socket);
    let mut conn = ClientConnection::new(
        crate::tls::client(certificate)?,
        "cine-relay.local".try_into().map_err(|_| Error::Tls)?,
    )
    .map_err(|_| Error::Tls)?;
    conn.set_buffer_limit(Some(64 * 1024));
    let mut pipe = StreamOwned::new(
        conn,
        BoundedSocket {
            socket,
            until: Some(Instant::now() + Duration::from_secs(10)),
            control: control.clone(),
        },
    );
    transport::send_control(&mut pipe, ticket)?;
    let mut accepted = [0; 4];
    pipe.read_exact(&mut accepted)?;
    if &accepted != b"CVR1" {
        return Err(Error::Unauthorized);
    }
    pipe.sock.until = None;
    pipe.configure(Duration::from_secs(3))?;
    Ok(pipe)
}

#[derive(Clone, Copy)]
pub struct Limits {
    /// Aggregate inner ciphertext, both directions, including peer handshakes.
    pub bytes: u64,
    pub duration: Duration,
    pub idle: Duration,
    pub bytes_per_second: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            duration: Duration::from_secs(60),
            idle: Duration::from_secs(3),
            bytes_per_second: 8 * 1024 * 1024,
        }
    }
}
#[derive(Default, Serialize)]
pub struct Stats {
    pub forwarded_ciphertext_bytes: u64,
    pub elapsed_seconds: f64,
    /// A relay session ending never asserts file completion or integrity.
    pub termination: &'static str,
}
fn io_error(stage: &str, error: std::io::Error) -> Error {
    // Debug diagnostics contain only static stage and error categories, never
    // endpoint, ticket, certificate, TLS error text or media metadata.
    #[cfg(debug_assertions)]
    eprintln!(
        "relay_io stage={stage} kind={:?} os={:?} tls={}",
        error.kind(),
        error.raw_os_error(),
        error.get_ref().is_some_and(|e| e.is::<rustls::Error>())
    );
    #[cfg(not(debug_assertions))]
    let _ = (stage, error);
    Error::Io
}
fn disconnected(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::NotConnected
    )
}

type Incoming = StreamOwned<ServerConnection, BoundedSocket>;
fn accept(
    listener: &TcpListener,
    config: Arc<rustls::ServerConfig>,
    control: &Control,
    until: Instant,
) -> Result<Incoming, Error> {
    loop {
        control.check()?;
        if Instant::now() >= until {
            return Err(Error::Expired);
        }
        match listener.accept() {
            Ok((socket, _)) => {
                socket.configure(Duration::from_secs(3))?;
                let mut conn = ServerConnection::new(config).map_err(|_| Error::Tls)?;
                conn.set_buffer_limit(Some(64 * 1024));
                return Ok(StreamOwned::new(
                    conn,
                    BoundedSocket {
                        socket,
                        until: Some(until.min(Instant::now() + Duration::from_secs(5))),
                        control: control.clone(),
                    },
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(_) => return Err(Error::Io),
        }
    }
}

/// Local laboratory gate: exactly two admissions, one pair, finite budget.
/// The caller provisions both tickets privately. RoomService integration is pending.
/// Fails closed after a bad/duplicate admission instead of accepting unlimited sockets.
pub fn serve_pair(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    sender: &Authorization,
    receiver: &Authorization,
    control: &Control,
    limits: Limits,
) -> Result<Stats, Error> {
    let ip = listener.local_addr()?.ip();
    let local = ip.is_loopback() || matches!(ip, std::net::IpAddr::V4(v4) if v4.is_private());
    if !local
        || limits.bytes == 0
        || limits.bytes > 128 * 1024 * 1024
        || limits.duration.is_zero()
        || limits.duration > Duration::from_secs(300)
        || limits.idle.is_zero()
        || limits.idle > Duration::from_secs(10)
        || limits.bytes_per_second == 0
        || limits.bytes_per_second > 8 * 1024 * 1024
    {
        return Err(Error::Bounds);
    }
    listener.set_nonblocking(true)?;
    let until = Instant::now() + Duration::from_secs(10);
    let mut sides = [None, None];
    for _ in 0..2 {
        sender.check()?;
        receiver.check()?;
        let mut stream = accept(&listener, config.clone(), control, until)?;
        let ticket = transport::read_control(&mut stream)?;
        // The grant ID only selects the verifier; scope and constant-time secret
        // comparison still run in Authorization::consume, including replay checks.
        let side = if ticket.grant_id == sender.credential.grant_id {
            sender.consume(&ticket)?;
            0
        } else {
            receiver.consume(&ticket)?;
            1
        };
        if sides[side].is_some() {
            return Err(Error::Replay);
        }
        sides[side] = Some(stream);
    }
    let [Some(mut a), Some(mut b)] = sides else {
        return Err(Error::Unauthorized);
    };
    for side in [&mut a, &mut b] {
        side.write_all(b"CVR1")?;
        side.flush()?;
        side.sock.until = None;
        side.sock.configure(Duration::from_secs(3))?;
    }
    let start = Instant::now();
    let mut last = start;
    let mut bytes = 0u64;
    let mut buffer = [0; 16 * 1024];
    loop {
        control.check()?;
        sender.check()?;
        receiver.check()?;
        if start.elapsed() >= limits.duration || last.elapsed() >= limits.idle {
            return Err(Error::Expired);
        }
        for side in 0..2 {
            let (source, destination) = if side == 0 {
                (&mut a, &mut b)
            } else {
                (&mut b, &mut a)
            };
            source
                .sock
                .socket
                .set_read_timeout(Some(Duration::from_millis(1)))
                .map_err(|e| io_error("poll_config", e))?;
            match source.read(&mut buffer) {
                Ok(0) => {
                    return Ok(Stats {
                        forwarded_ciphertext_bytes: bytes,
                        elapsed_seconds: start.elapsed().as_secs_f64(),
                        termination: "peer_closed",
                    });
                }
                Ok(n) => {
                    let next = bytes.checked_add(n as u64).ok_or(Error::Bounds)?;
                    if next > limits.bytes {
                        return Err(Error::Bounds);
                    }
                    if let Err(error) = destination
                        .write_all(&buffer[..n])
                        .and_then(|()| destination.flush())
                    {
                        // Some kernels reset a socket when its owner exits with
                        // unread encrypted termination records. Report lifecycle,
                        // not successful delivery: receiver SHA remains decisive.
                        control.check()?;
                        sender.check()?;
                        receiver.check()?;
                        if disconnected(&error) {
                            return Ok(Stats {
                                forwarded_ciphertext_bytes: bytes,
                                elapsed_seconds: start.elapsed().as_secs_f64(),
                                termination: "peer_disconnected",
                            });
                        }
                        return Err(io_error("forward", error));
                    }
                    bytes = next;
                    last = Instant::now();
                    let earliest =
                        Duration::from_secs_f64(bytes as f64 / limits.bytes_per_second as f64);
                    while start.elapsed() < earliest {
                        control.check()?;
                        sender.check()?;
                        receiver.check()?;
                        if start.elapsed() >= limits.duration {
                            return Err(Error::Expired);
                        }
                        std::thread::sleep(
                            earliest
                                .saturating_sub(start.elapsed())
                                .min(Duration::from_millis(2)),
                        );
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                // Unclean EOF/reset is a transport end, not file completion.
                Err(e) if disconnected(&e) => {
                    control.check()?;
                    sender.check()?;
                    receiver.check()?;
                    return Ok(Stats {
                        forwarded_ciphertext_bytes: bytes,
                        elapsed_seconds: start.elapsed().as_secs_f64(),
                        termination: "peer_disconnected",
                    });
                }
                Err(e) => return Err(io_error("read", e)),
            }
        }
    }
}
