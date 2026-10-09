//! Application owns transfer workers; room authorization and binary I/O stay separate.
use cine_rooms::model::RoomState;
use cine_transfer::{
    Authorization, Control, Error, Progress, storage::Partial, tls::Identity, transport,
};
use cine_transfer_model::{CHUNK_SIZE, Grant, Manifest, Offer, TransferSnapshot};
use serde_json::{Value, json};
use std::{
    fs::File,
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
#[derive(Default)]
struct Public {
    progress: Progress,
    completed: Option<PathBuf>,
}
struct Worker {
    control: Control,
    stop: Arc<AtomicBool>,
    socket: Arc<Mutex<Option<TcpStream>>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Worker {
    fn interrupt(&self, cancel: bool) {
        if cancel {
            self.control.cancel.store(true, Ordering::Release)
        } else {
            self.control.pause.store(true, Ordering::Release)
        }
        if let Some(s) = self.socket.lock().unwrap().as_ref() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.interrupt(true);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
struct Host {
    worker: Worker,
    grant: Arc<Mutex<Option<Arc<Authorization>>>>,
    manifest: Manifest,
    pending_offer: bool,
}
struct Download {
    worker: Worker,
    grants: mpsc::SyncSender<Grant>,
    manifest: Manifest,
    root: PathBuf,
}
pub struct Transfers {
    pub supported: bool,
    pub snapshot: TransferSnapshot,
    host: Option<Host>,
    download: Option<Download>,
    public: Arc<Mutex<Public>>,
}
impl Default for Transfers {
    fn default() -> Self {
        Self {
            supported: false,
            snapshot: TransferSnapshot {
                offer: None,
                receivers: vec![],
            },
            host: None,
            download: None,
            public: Arc::new(Mutex::new(Public::default())),
        }
    }
}
impl Transfers {
    pub fn summary(&self) -> Value {
        let p = self.public.lock().unwrap();
        // Cert, IP, digest, credential and local completed path never enter UI diagnostics.
        json!({"supported":self.supported,"offer":self.snapshot.offer.as_ref().map(|o|json!({"transfer_id":o.manifest.transfer_id,"size_bytes":o.manifest.size_bytes,"display_name":o.manifest.display_name,"duration_ms":o.manifest.duration_ms})),"receivers":self.snapshot.receivers,"progress":p.progress})
    }
    pub fn progress(&self) -> Progress {
        self.public.lock().unwrap().progress.clone()
    }
    pub fn completed(&self) -> Option<PathBuf> {
        self.public.lock().unwrap().completed.clone()
    }
    pub fn host(
        &mut self,
        address: SocketAddr,
        manifest: Manifest,
        mut file: File,
    ) -> Result<Offer, Error> {
        if !self.supported {
            return Err(Error::Unauthorized);
        }
        manifest.validate()?;
        self.host = None;
        let identity = Identity::generate()?;
        let listener = TcpListener::bind(address)?;
        listener.set_nonblocking(true)?;
        let offer = Offer {
            manifest: manifest.clone(),
            address: listener.local_addr()?,
            certificate: identity.certificate,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let control = Control::default();
        let socket = Arc::new(Mutex::new(None));
        let grant = Arc::new(Mutex::new(None::<Arc<Authorization>>));
        let pending = grant.clone();
        let stopping = stop.clone();
        let ctl = control.clone();
        let active = socket.clone();
        let public = self.public.clone();
        let join = thread::Builder::new()
            .name("cine-transfer-send".into())
            .spawn(move || {
                let mut window = Instant::now();
                let mut attempts = 0;
                while !stopping.load(Ordering::Acquire) {
                    if window.elapsed() > Duration::from_secs(60) {
                        window = Instant::now();
                        attempts = 0;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            attempts += 1;
                            if attempts > 12 {
                                drop(stream);
                                continue;
                            }
                            // Both private grants are delivered independently. TCP can
                            // arrive before the Host's WS owner processes its grant.
                            // Keep only this one socket for a bounded rendezvous.
                            let waiting = Instant::now();
                            let auth = loop {
                                if let Some(auth) = pending
                                    .lock()
                                    .unwrap()
                                    .clone()
                                    .filter(|auth| auth.available())
                                {
                                    break Some(auth);
                                }
                                if stopping.load(Ordering::Acquire)
                                    || waiting.elapsed() >= Duration::from_secs(2)
                                {
                                    break None;
                                }
                                thread::sleep(Duration::from_millis(10));
                            };
                            let Some(auth) = auth else {
                                drop(stream);
                                continue;
                            };
                            if auth.check().is_err() {
                                continue;
                            }
                            *active.lock().unwrap() = stream.try_clone().ok();
                            let result = transport::send(
                                stream,
                                identity.server.clone(),
                                &manifest,
                                &mut file,
                                &auth,
                                &ctl,
                                |p| public.lock().unwrap().progress = p,
                            );
                            *active.lock().unwrap() = None;
                            if let Err(error) = result {
                                public.lock().unwrap().progress.error = Some(error.to_string());
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|_| Error::Io)?;
        self.host = Some(Host {
            manifest: offer.manifest.clone(),
            pending_offer: true,
            worker: Worker {
                control,
                stop,
                socket,
                join: Some(join),
            },
            grant,
        });
        Ok(offer)
    }
    pub fn download(&mut self, root: &Path) -> Result<Uuid, Error> {
        if !self.supported {
            return Err(Error::Unauthorized);
        }
        let offer = self.snapshot.offer.as_ref().ok_or(Error::Manifest)?.clone();
        let root = root.canonicalize().map_err(|_| Error::Storage)?;
        if self.download.as_ref().is_some_and(|d| {
            d.manifest == offer.manifest
                && d.root == root
                && !d.worker.control.cancel.load(Ordering::Acquire)
                && d.worker.join.as_ref().is_some_and(|j| !j.is_finished())
        }) {
            self.waiting();
            return Ok(offer.manifest.transfer_id);
        }
        self.download = None;
        let partial = Partial::create(&root, offer.manifest.clone())?;
        let control = Control::default();
        let stop = Arc::new(AtomicBool::new(false));
        let socket = Arc::new(Mutex::new(None));
        let ctl = control.clone();
        let stopping = stop.clone();
        let active = socket.clone();
        let public = self.public.clone();
        let (tx, rx) = mpsc::sync_channel::<Grant>(1);
        {
            let mut p = public.lock().unwrap();
            p.completed = None;
            p.progress = Progress {
                state: "waiting_for_acceptance".into(),
                total_bytes: offer.manifest.size_bytes,
                ..Progress::default()
            };
        }
        let join = thread::Builder::new()
            .name("cine-transfer-receive".into())
            .spawn(move || {
                let mut partial = partial;
                while !stopping.load(Ordering::Acquire) {
                    if ctl.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    let grant = match rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(g) => g,
                        Err(mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => break,
                    };
                    if ctl.cancel.load(Ordering::Acquire) {
                        break;
                    }
                    ctl.pause.store(false, Ordering::Release);
                    public.lock().unwrap().progress.state = "connecting".into();
                    let result = transport::receive_observed(
                        offer.address,
                        &offer.certificate,
                        &grant.credential,
                        &mut partial,
                        &ctl,
                        |s| *active.lock().unwrap() = s.try_clone().ok(),
                        |p| public.lock().unwrap().progress = p,
                    );
                    *active.lock().unwrap() = None;
                    let mut p = public.lock().unwrap();
                    p.progress.bytes_per_second = None;
                    p.progress.eta_seconds = None;
                    match result {
                        Ok(path) => {
                            p.completed = Some(path);
                            p.progress.state = "completed".into();
                            p.progress.error = None;
                            break;
                        }
                        Err(e) => {
                            let e = ctl.check().err().unwrap_or(e);
                            p.progress.state = match e {
                                Error::Paused => "paused",
                                Error::Cancelled => "cancelled",
                                Error::Io => "reconnecting",
                                _ => "failed",
                            }
                            .into();
                            p.progress.error = (!matches!(e, Error::Paused | Error::Cancelled))
                                .then_some(e.to_string());
                        }
                    }
                }
                if ctl.cancel.load(Ordering::Acquire) {
                    let cleanup_failed = !partial.completed() && partial.discard().is_err();
                    let mut p = public.lock().unwrap();
                    p.completed = None;
                    p.progress.state = "cancelled".into();
                    p.progress.verified_bytes = 0;
                    p.progress.received_bytes = 0;
                    p.progress.error = cleanup_failed.then_some(Error::Storage.to_string());
                }
            })
            .map_err(|_| Error::Io)?;
        self.download = Some(Download {
            worker: Worker {
                control,
                stop,
                socket,
                join: Some(join),
            },
            grants: tx,
            manifest: offer.manifest.clone(),
            root,
        });
        Ok(offer.manifest.transfer_id)
    }
    pub fn pause(&self) {
        if let Some(d) = &self.download {
            d.worker.interrupt(false);
        }
    }
    pub fn abort_host(&mut self) {
        self.host = None;
    }
    pub fn waiting(&self) {
        let mut p = self.public.lock().unwrap();
        p.progress.state = "waiting_for_acceptance".into();
        p.progress.error = None;
    }
    pub fn cancel(&mut self) {
        if let Some(d) = &self.download {
            d.worker.interrupt(true);
            self.public.lock().unwrap().progress.state = "cancelled".into();
        }
        self.public.lock().unwrap().completed = None;
    }
    pub fn disconnected(&self) {
        self.pause();
        if let Some(h) = &self.host {
            if let Some(auth) = h.grant.lock().unwrap().as_ref() {
                auth.revoked.store(true, Ordering::Release)
            }
            h.worker.interrupt(false);
        }
    }
    pub fn reconcile(&mut self, room: &RoomState, member: Uuid) {
        let matches = |manifest: &Manifest| {
            room.room_id == manifest.room_id
                && room.room_epoch == manifest.room_epoch
                && room.host_id == manifest.host_id
                && room.authority_revision == manifest.authority_revision
                && room.media.as_ref().is_some_and(|m| {
                    m.media_revision == manifest.media_revision
                        && m.descriptor.identity == manifest.identity()
                })
        };
        let valid = self
            .snapshot
            .offer
            .as_ref()
            .is_some_and(|o| matches(&o.manifest));
        if !valid {
            self.snapshot.offer = None;
            self.cancel();
        }
        // Room snapshots may race the offer's ACK/broadcast. Pending listeners
        // stay private and unauthenticated; only a valid grant enables bytes.
        if let Some(h) = &self.host
            && !matches(&h.manifest)
        {
            h.worker.stop.store(true, Ordering::Release);
            h.worker.interrupt(true);
        }
        if !room
            .members
            .iter()
            .any(|m| m.member_id == room.host_id && m.connected)
            || !room
                .members
                .iter()
                .any(|m| m.member_id == member && m.connected)
        {
            self.disconnected();
        }
    }
    pub fn install(
        &mut self,
        snapshot: TransferSnapshot,
        grant: Option<Grant>,
        room: &RoomState,
        member: Uuid,
        server_now: u64,
    ) -> Result<(), Error> {
        if !self.supported || snapshot.receivers.len() > 15 {
            return Err(Error::Unauthorized);
        }
        if let Some(o) = &snapshot.offer {
            o.manifest.validate()?;
            if o.certificate.len() > 2048 {
                return Err(Error::Manifest);
            }
        }
        self.snapshot = snapshot;
        if let Some(h) = &mut self.host {
            if self
                .snapshot
                .offer
                .as_ref()
                .is_some_and(|o| o.manifest == h.manifest)
            {
                h.pending_offer = false;
            } else if !h.pending_offer {
                h.worker.stop.store(true, Ordering::Release);
                h.worker.interrupt(true);
            }
        }
        self.reconcile(room, member);
        if let Some(h) = &self.host
            && let Some(auth) = h.grant.lock().unwrap().as_ref()
            && !self
                .snapshot
                .receivers
                .iter()
                .any(|r| r.authorized && r.state != "paused" && r.state != "cancelled")
        {
            auth.revoked.store(true, Ordering::Release);
            h.worker.interrupt(false);
        }
        if grant.is_none()
            && let Some(d) = &self.download
        {
            let state = &self.public.lock().unwrap().progress.state;
            if ["connecting", "transferring", "verifying"].contains(&state.as_str())
                && !self
                    .snapshot
                    .receivers
                    .iter()
                    .any(|r| r.receiver_id == member && r.authorized)
            {
                d.worker.interrupt(false);
            }
        }
        if let Some(g) = grant {
            let offer = self.snapshot.offer.as_ref().ok_or(Error::Unauthorized)?;
            if g.expires_at_ms <= server_now
                || g.expires_at_ms - server_now > 600_000
                || g.credential.manifest_hash != offer.manifest.fingerprint()
            {
                return Err(Error::Unauthorized);
            }
            if member == room.host_id {
                let h = self.host.as_ref().ok_or(Error::Unauthorized)?;
                h.worker.control.pause.store(false, Ordering::Release);
                *h.grant.lock().unwrap() = Some(Arc::new(Authorization::new(
                    g.credential,
                    Duration::from_millis(g.expires_at_ms - server_now),
                )));
            } else {
                if g.credential.receiver_id != member {
                    return Err(Error::Unauthorized);
                }
                let d = self.download.as_ref().ok_or(Error::Unauthorized)?;
                if d.manifest != offer.manifest {
                    return Err(Error::Unauthorized);
                }
                *d.worker.control.deadline.lock().unwrap() =
                    Some(Instant::now() + Duration::from_millis(g.expires_at_ms - server_now));
                d.grants.try_send(g).map_err(|_| Error::Replay)?;
            }
        }
        Ok(())
    }
}
pub fn manifest(room: &RoomState) -> Result<Manifest, Error> {
    let m = room.media.as_ref().ok_or(Error::Manifest)?;
    Ok(Manifest {
        version: 1,
        transfer_id: Uuid::new_v4(),
        room_id: room.room_id,
        room_epoch: room.room_epoch,
        host_id: room.host_id,
        authority_revision: room.authority_revision,
        media_revision: m.media_revision,
        media_id: m.descriptor.media_id.parse().map_err(|_| Error::Manifest)?,
        display_name: "Película del anfitrión".into(),
        size_bytes: m.descriptor.identity.size_bytes,
        chunk_size: CHUNK_SIZE,
        chunk_count: m
            .descriptor
            .identity
            .size_bytes
            .div_ceil(u64::from(CHUNK_SIZE)) as u32,
        sha256: m.descriptor.identity.sha256,
        duration_ms: m.descriptor.duration_ms,
        block_hash: "sha256".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cine_core::media::{ContentIdentity, MediaDescriptor, SourceType};
    use cine_rooms::model::{MediaSelection, Member, MemberStatus, Role};
    #[test]
    fn accepted_socket_waits_for_fresh_grant_when_previous_grant_is_revoked() {
        let root = std::env::temp_dir().join(format!("cine-fresh-grant-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source");
        std::fs::write(&source, b"abc").unwrap();
        let mut file = File::open(&source).unwrap();
        let identity = cine_local_media::hash_reader(&mut file, None, |_| true).unwrap();
        let manifest = Manifest {
            version: 1,
            transfer_id: Uuid::new_v4(),
            room_id: Uuid::new_v4(),
            room_epoch: Uuid::new_v4(),
            host_id: Uuid::new_v4(),
            authority_revision: 1,
            media_revision: 1,
            media_id: Uuid::new_v4(),
            display_name: "fixture".into(),
            size_bytes: 3,
            chunk_size: CHUNK_SIZE,
            chunk_count: 1,
            sha256: identity.sha256,
            duration_ms: 1000,
            block_hash: "sha256".into(),
        };
        let mut t = Transfers {
            supported: true,
            ..Default::default()
        };
        let offer = t
            .host("127.0.0.1:0".parse().unwrap(), manifest.clone(), file)
            .unwrap();
        let old = Arc::new(Authorization::new(
            cine_transfer::Credential::new(&manifest, Uuid::new_v4()).unwrap(),
            Duration::from_secs(10),
        ));
        old.revoked.store(true, Ordering::Release);
        let grants = t.host.as_ref().unwrap().grant.clone();
        *grants.lock().unwrap() = Some(old);
        let fresh = cine_transfer::Credential::new(&manifest, Uuid::new_v4()).unwrap();
        let receiver_credential = fresh.clone();
        let destination = root.clone();
        let (connected_tx, connected_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let receiver = thread::spawn(move || {
            let mut partial = Partial::create(&destination, manifest).unwrap();
            let result = transport::receive_observed(
                offer.address,
                &offer.certificate,
                &receiver_credential,
                &mut partial,
                &Control::default(),
                |_| {
                    connected_tx.send(()).unwrap();
                },
                |_| {},
            );
            done_tx.send(result).unwrap();
        });
        connected_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        // No sleeps: the receiver must remain pending while its WSS grant has
        // not yet arrived at the Host. Old code returns Io in this window.
        assert!(matches!(
            done_rx.recv_timeout(Duration::from_millis(200)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        *grants.lock().unwrap() =
            Some(Arc::new(Authorization::new(fresh, Duration::from_secs(10))));
        let path = done_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"abc");
        receiver.join().unwrap();
        drop(t);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pending_offer_survives_room_snapshot_but_media_change_and_withdraw_close_listener() {
        let host = Uuid::new_v4();
        let mut room = RoomState {
            room_id: Uuid::new_v4(),
            room_epoch: Uuid::new_v4(),
            sequence: 1,
            updated_at_ms: 0,
            host_id: host,
            authority_revision: 1,
            members: vec![Member {
                member_id: host,
                display_name: "fixture".into(),
                role: Role::Host,
                connected: true,
                ready: false,
                verified_media_revision: None,
                status: MemberStatus::Idle,
                joined_at_ms: 0,
                lease_expires_at_ms: None,
            }],
            media: Some(MediaSelection {
                media_revision: 1,
                descriptor: MediaDescriptor {
                    media_id: Uuid::new_v4().to_string(),
                    source_type: SourceType::LocalFile,
                    title: None,
                    duration_ms: 1000,
                    identity: ContentIdentity {
                        size_bytes: 3,
                        sha256: [1; 32],
                    },
                    mime: None,
                    codecs: vec![],
                },
            }),
            playback: None,
        };
        let path = std::env::temp_dir().join(format!("cine-transfer-source-{}", Uuid::new_v4()));
        std::fs::write(&path, b"abc").unwrap();
        let mut t = Transfers {
            supported: true,
            ..Default::default()
        };
        let offer = t
            .host(
                "127.0.0.1:0".parse().unwrap(),
                manifest(&room).unwrap(),
                File::open(&path).unwrap(),
            )
            .unwrap();
        t.reconcile(&room, host);
        assert!(!t.host.as_ref().unwrap().worker.stop.load(Ordering::Acquire));
        assert!(t.snapshot.offer.is_none()); // no optimistic public offer or authorization
        t.install(
            TransferSnapshot {
                offer: Some(offer),
                receivers: vec![],
            },
            None,
            &room,
            host,
            0,
        )
        .unwrap();
        assert!(!t.host.as_ref().unwrap().pending_offer);
        t.install(
            TransferSnapshot {
                offer: None,
                receivers: vec![],
            },
            None,
            &room,
            host,
            0,
        )
        .unwrap();
        assert!(t.host.as_ref().unwrap().worker.stop.load(Ordering::Acquire));
        t.host(
            "127.0.0.1:0".parse().unwrap(),
            manifest(&room).unwrap(),
            File::open(&path).unwrap(),
        )
        .unwrap();
        room.media.as_mut().unwrap().media_revision += 1;
        t.reconcile(&room, host);
        assert!(t.host.as_ref().unwrap().worker.stop.load(Ordering::Acquire));
        drop(t);
        std::fs::remove_file(path).unwrap();
    }
}
