use crate::{
    Authorization, Control, Credential, Error, MAX_CONTROL, Manifest, Progress, storage::Partial,
};
use rustls::{ClientConnection, ServerConnection, StreamOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    net::{SocketAddr, TcpStream},
    sync::Arc,
    time::{Duration, Instant},
};
struct DeadlineSocket<'a> {
    stream: TcpStream,
    handshake_until: Option<Instant>,
    control: &'a Control,
    auth: Option<&'a Authorization>,
}
impl DeadlineSocket<'_> {
    fn check(&self, writing: bool) -> std::io::Result<()> {
        self.control
            .check()
            .map_err(|_| std::io::ErrorKind::ConnectionAborted)?;
        if let Some(auth) = self.auth {
            auth.check()
                .map_err(|_| std::io::ErrorKind::PermissionDenied)?;
        }
        let timeout = if let Some(until) = self.handshake_until {
            let remaining = until.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::ErrorKind::TimedOut.into());
            }
            remaining.min(Duration::from_secs(3))
        } else {
            return Ok(());
        };
        if writing {
            self.stream.set_write_timeout(Some(timeout))
        } else {
            self.stream.set_read_timeout(Some(timeout))
        }
    }
}
impl Read for DeadlineSocket<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.check(false)?;
        self.stream.read(buf)
    }
}
impl Write for DeadlineSocket<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.check(true)?;
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.check(true)?;
        self.stream.flush()
    }
}
fn socket(stream: &TcpStream) -> Result<(), Error> {
    // The listener polls nonblocking. Accepted sockets can inherit that mode
    // on Windows/BSD; this worker requires blocking I/O with bounded timeouts.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.set_nodelay(true)?;
    Ok(())
}
fn send_control(s: &mut impl Write, c: &Credential) -> Result<(), Error> {
    let data = serde_json::to_vec(c).map_err(|_| Error::Unauthorized)?;
    if data.len() > MAX_CONTROL {
        return Err(Error::Unauthorized);
    }
    s.write_all(&(data.len() as u32).to_be_bytes())?;
    s.write_all(&data)?;
    s.flush()?;
    Ok(())
}
fn read_control(s: &mut impl Read) -> Result<Credential, Error> {
    let mut len = [0; 4];
    s.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;
    if len > MAX_CONTROL {
        return Err(Error::Unauthorized);
    }
    let mut data = vec![0; len];
    s.read_exact(&mut data)?;
    serde_json::from_slice(&data).map_err(|_| Error::Unauthorized)
}
pub fn send(
    stream: TcpStream,
    config: Arc<rustls::ServerConfig>,
    manifest: &Manifest,
    file: &mut File,
    auth: &Authorization,
    control: &Control,
    mut progress: impl FnMut(Progress),
) -> Result<(), Error> {
    manifest.validate()?;
    socket(&stream)?;
    let before = file.metadata()?;
    if !before.is_file() || before.len() != manifest.size_bytes {
        return Err(Error::Modified);
    }
    let mut conn = ServerConnection::new(config).map_err(|_| Error::Tls)?;
    conn.set_buffer_limit(Some(64 * 1024));
    let mut tls = StreamOwned::new(
        conn,
        DeadlineSocket {
            stream,
            handshake_until: Some(Instant::now() + Duration::from_secs(5)),
            control,
            auth: Some(auth),
        },
    );
    let candidate = read_control(&mut tls)?;
    auth.consume(&candidate)?;
    if candidate.manifest_hash != manifest.fingerprint() {
        return Err(Error::Unauthorized);
    }
    tls.write_all(b"CVP1")?;
    tls.flush()?;
    tls.sock.handshake_until = None;
    socket(&tls.sock.stream)?;
    let mut data = vec![0; manifest.chunk_size as usize];
    let mut transferred = 0;
    let start = Instant::now();
    loop {
        control.check()?;
        auth.check()?;
        let mut request = [0; 4];
        tls.read_exact(&mut request)?;
        let index = u32::from_be_bytes(request);
        if index == u32::MAX {
            break;
        }
        let (offset, len) = manifest.chunk(index)?;
        let current = file.metadata()?;
        if current.len() != before.len() || current.modified().ok() != before.modified().ok() {
            return Err(Error::Modified);
        }
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut data[..len])?;
        auth.check()?;
        control.check()?;
        let hash: [u8; 32] = Sha256::digest(&data[..len]).into();
        tls.write_all(&index.to_be_bytes())?;
        tls.write_all(&offset.to_be_bytes())?;
        tls.write_all(&(len as u32).to_be_bytes())?;
        tls.write_all(&hash)?;
        tls.write_all(&data[..len])?;
        tls.flush()?;
        transferred += len as u64;
        // Conservative MVP cap: preserve network/CPU headroom for the Player.
        // One sender worker sleeps cooperatively; no runtime or UI thread sleeps.
        let earliest = Duration::from_secs_f64(transferred as f64 / (8.0 * 1024.0 * 1024.0));
        while start.elapsed() < earliest {
            control.check()?;
            auth.check()?;
            std::thread::sleep(
                earliest
                    .saturating_sub(start.elapsed())
                    .min(Duration::from_millis(10)),
            );
        }

        progress(Progress {
            state: "transferring".into(),
            received_bytes: transferred,
            total_bytes: manifest.size_bytes,
            bytes_per_second: Some(transferred as f64 / start.elapsed().as_secs_f64().max(0.001)),
            ..Progress::default()
        });
    }
    tls.conn.send_close_notify();
    let _ = tls.flush();
    Ok(())
}
pub fn receive_observed(
    address: SocketAddr,
    certificate: &[u8],
    credential: &Credential,
    partial: &mut Partial,
    control: &Control,
    mut connected: impl FnMut(&TcpStream),
    mut progress: impl FnMut(Progress),
) -> Result<std::path::PathBuf, Error> {
    control.check()?;
    if credential.manifest_hash != partial.manifest.fingerprint() {
        return Err(Error::Unauthorized);
    }
    partial.revalidate(|| control.check())?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    socket(&stream)?;
    connected(&stream);
    let mut conn = ClientConnection::new(
        crate::tls::client(certificate)?,
        "cine-transfer.local".try_into().map_err(|_| Error::Tls)?,
    )
    .map_err(|_| Error::Tls)?;
    conn.set_buffer_limit(Some(64 * 1024));
    let mut tls = StreamOwned::new(
        conn,
        DeadlineSocket {
            stream,
            handshake_until: Some(Instant::now() + Duration::from_secs(5)),
            control,
            auth: None,
        },
    );
    send_control(&mut tls, credential)?;
    let mut accept = [0; 4];
    tls.read_exact(&mut accept)?;
    if &accept != b"CVP1" {
        return Err(Error::Unauthorized);
    }
    tls.sock.handshake_until = None;
    socket(&tls.sock.stream)?;
    let mut data = vec![0; partial.manifest.chunk_size as usize];
    let start = Instant::now();
    let initial = partial.verified;
    let mut received = 0;
    let indices: Vec<_> = partial.missing().collect(); // <=16384 indices, never one future per block.
    for expected_index in indices {
        control.check()?;
        tls.write_all(&expected_index.to_be_bytes())?;
        tls.flush()?;
        let mut header = [0; 48];
        tls.read_exact(&mut header)?;
        let index = u32::from_be_bytes(header[..4].try_into().unwrap());
        let offset = u64::from_be_bytes(header[4..12].try_into().unwrap());
        let length = u32::from_be_bytes(header[12..16].try_into().unwrap()) as usize;
        let hash = header[16..48].try_into().unwrap();
        let (expected_offset, expected_length) = partial.manifest.chunk(expected_index)?;
        if index != expected_index || offset != expected_offset || length != expected_length {
            return Err(Error::Bounds);
        }
        tls.read_exact(&mut data[..length])?;
        control.check()?;
        received += length as u64;
        partial.accept(index, offset, &data[..length], hash)?;
        let speed = (partial.verified - initial) as f64 / start.elapsed().as_secs_f64().max(0.001);
        progress(Progress {
            state: "transferring".into(),
            received_bytes: received,
            verified_bytes: partial.verified,
            total_bytes: partial.manifest.size_bytes,
            bytes_per_second: Some(speed),
            eta_seconds: (speed > 0.0)
                .then_some((partial.manifest.size_bytes - partial.verified) as f64 / speed),
            error: None,
        });
    }
    tls.write_all(&u32::MAX.to_be_bytes())?;
    tls.flush()?;
    drop(tls);
    progress(Progress {
        state: "verifying".into(),
        received_bytes: received,
        verified_bytes: partial.verified,
        total_bytes: partial.manifest.size_bytes,
        ..Progress::default()
    });
    partial.finish(|| control.check())
}

pub fn receive(
    address: SocketAddr,
    certificate: &[u8],
    credential: &Credential,
    partial: &mut Partial,
    control: &Control,
    progress: impl FnMut(Progress),
) -> Result<std::path::PathBuf, Error> {
    receive_observed(
        address,
        certificate,
        credential,
        partial,
        control,
        |_| {},
        progress,
    )
}
