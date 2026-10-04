//! Experimental Application boundary: DTOs, never a Rust struct ABI or SDK object.
use cine_core::{
    clock::{ClockFilter, ClockSample},
    sync::{Correction, Observation, SyncConfig, SyncEngine},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

const LIMIT: usize = 65_536;
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub api_version: u32,
    pub generation: u64,
    pub command: Command,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Command {
    State,
    Configure(PlayerCapabilities),
    Sample(Sample),
    Play,
    Pause,
    Seek { position_ms: u64 },
    Rate { rate: f64 },
    Suspend,
    Resume,
    Snapshot,
    Clock { t1: i64, t2: i64, t3: i64, t4: i64 },
    SimulateDrift { drift_ms: i64 },
    Observe { target_ms: u64 },
    SchedulePlay { deadline_ms: u64 },
    CancelHash,
    PlayerError,
}
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerCapabilities {
    pub playback_rate: bool,
    pub content_uri_input: bool,
}
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub playing: bool,
    pub loaded: bool,
    pub buffering: bool,
    pub seeking: bool,
    pub age_ms: u64,
}
#[derive(Debug, Clone, Serialize)]
pub struct Effect {
    pub action: &'static str,
    pub value: f64,
    pub generation: u64,
}
#[derive(Debug, Serialize)]
pub struct Failure {
    pub code: &'static str,
    pub message: &'static str,
}
#[derive(Debug, Serialize)]
pub struct Reply {
    pub api_version: u32,
    pub ok: bool,
    pub error: Option<Failure>,
    pub generation: u64,
    pub now_ms: u64,
    pub suspended: bool,
    pub clock_trusted: bool,
    pub snapshot_required: bool,
    pub sample: Sample,
    pub capabilities: PlayerCapabilities,
    pub effects: Vec<Effect>,
    pub hash: HashStatus,
}
#[derive(Debug, Default, Clone, Serialize)]
pub struct HashStatus {
    pub state: &'static str,
    pub read_bytes: u64,
    pub size_bytes: u64,
    pub elapsed_ms: f64,
    // Identity remains device-local. Logs/results must not include it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
struct HashJob {
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<HashStatus>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for HashJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub struct Application {
    generation: u64,
    suspended: bool,
    snapshot_required: bool,
    clock: ClockFilter,
    last_clock: Option<u64>,
    clock_samples: Vec<(f64, f64)>,
    sample: Sample,
    caps: PlayerCapabilities,
    sync: SyncEngine,
    deadline: Option<u64>,
    job: Option<HashJob>,
}
impl Default for Application {
    fn default() -> Self {
        Self {
            generation: 1,
            suspended: false,
            snapshot_required: true,
            clock: ClockFilter::default(),
            last_clock: None,
            clock_samples: Vec::new(),
            sample: Sample::default(),
            caps: PlayerCapabilities::default(),
            sync: SyncEngine::new(SyncConfig::default()).unwrap(),
            deadline: None,
            job: None,
        }
    }
}
impl Application {
    fn trusted(&self, now: u64) -> bool {
        let Some(estimate) = self.clock.estimate() else {
            return false;
        };
        let mut best = self.clock_samples.clone();
        best.sort_by(|a, b| a.0.total_cmp(&b.0));
        best.truncate(3);
        let dispersion = best
            .iter()
            .map(|(_, offset)| (offset - estimate.offset_ms).abs())
            .fold(0.0, f64::max);
        !self.suspended
            && estimate.rtt_ms / 2.0 + dispersion <= 100.0
            && self
                .last_clock
                .is_some_and(|last| now.saturating_sub(last) <= 15_000)
    }
    fn reply(&self, now: u64, effects: Vec<Effect>, error: Option<&'static str>) -> Reply {
        Reply {
            api_version: 1,
            ok: error.is_none(),
            error: error.map(|code| Failure {
                code,
                message: code,
            }),
            generation: self.generation,
            now_ms: now,
            suspended: self.suspended,
            clock_trusted: self.trusted(now),
            snapshot_required: self.snapshot_required,
            sample: self.sample,
            capabilities: self.caps,
            effects,
            hash: self
                .job
                .as_ref()
                .map(|j| j.status.lock().unwrap().clone())
                .unwrap_or_default(),
        }
    }
    fn effect(&self, action: &'static str, value: f64) -> Effect {
        Effect {
            action,
            value,
            generation: self.generation,
        }
    }
    pub fn dispatch(&mut self, bytes: &[u8], now: u64) -> Reply {
        if bytes.len() > LIMIT {
            return self.reply(now, vec![], Some("PAYLOAD_TOO_LARGE"));
        }
        let Ok(request) = serde_json::from_slice::<Request>(bytes) else {
            return self.reply(now, vec![], Some("INVALID_DTO"));
        };
        if request.api_version != 1 {
            return self.reply(now, vec![], Some("VERSION_UNSUPPORTED"));
        }
        if request.generation != self.generation {
            return self.reply(now, vec![], Some("STALE_GENERATION"));
        }
        let mut effects = vec![];
        let mut error = None;
        match request.command {
            Command::State => {
                if self.deadline.is_some_and(|deadline| now >= deadline) {
                    self.deadline = None;
                    if self.trusted(now) && !self.snapshot_required && self.sample.loaded {
                        effects.push(self.effect("play", 0.0));
                    }
                }
            }
            Command::Configure(caps) => self.caps = caps,
            Command::Sample(sample) => {
                if sample.duration_ms > 604_800_000
                    || sample.position_ms > sample.duration_ms
                    || sample.age_ms > 100
                {
                    error = Some("INVALID_SAMPLE");
                } else {
                    self.sample = sample;
                }
            }
            Command::Play => {
                if self.suspended
                    || !self.sample.loaded
                    || self.sample.buffering
                    || self.sample.seeking
                {
                    error = Some("PLAYER_NOT_READY");
                } else {
                    effects.push(self.effect("play", 0.0));
                }
            }
            Command::Pause => effects.push(self.effect("pause", 0.0)),
            Command::Seek { position_ms } => {
                if !self.sample.loaded || self.suspended {
                    error = Some("PLAYER_NOT_READY");
                } else if position_ms > self.sample.duration_ms {
                    error = Some("SEEK_OUT_OF_RANGE");
                } else {
                    self.sync.reset();
                    if self.caps.playback_rate {
                        effects.push(self.effect("rate", 1.0));
                    }
                    effects.push(self.effect("seek", position_ms as f64));
                }
            }
            Command::Rate { rate } => {
                if !self.caps.playback_rate {
                    error = Some("RATE_UNSUPPORTED");
                } else if !rate.is_finite() || !(0.5..=2.0).contains(&rate) {
                    error = Some("INVALID_RATE");
                } else {
                    effects.push(self.effect("rate", rate));
                }
            }
            Command::Suspend | Command::Resume | Command::PlayerError => {
                self.suspended = matches!(request.command, Command::Suspend);
                self.generation += 1;
                self.clock = ClockFilter::default();
                self.last_clock = None;
                self.clock_samples.clear();
                self.snapshot_required = true;
                self.deadline = None;
                self.sync.reset();
                self.sample = Sample::default();
                if let Some(job) = &self.job {
                    job.cancel.store(true, Ordering::Release);
                }
                effects.push(self.effect("pause", 0.0));
                if self.caps.playback_rate {
                    effects.push(self.effect("rate", 1.0));
                }
            }
            Command::Snapshot => {
                if self.suspended {
                    error = Some("SUSPENDED");
                } else {
                    self.snapshot_required = false;
                    self.sync.reset();
                    if self.caps.playback_rate {
                        effects.push(self.effect("rate", 1.0));
                    }
                }
            }
            Command::Clock { t1, t2, t3, t4 } => {
                if self.suspended
                    || [t1, t2, t3, t4]
                        .iter()
                        .any(|t| !(0..=9_007_199_254_740_991).contains(t))
                    || !self.clock.push(ClockSample { t1, t2, t3, t4 })
                {
                    error = Some("INVALID_CLOCK_SAMPLE");
                } else {
                    self.last_clock = Some(now);
                    let estimate = ClockSample { t1, t2, t3, t4 }.estimate().unwrap();
                    if self.clock_samples.len() == 8 {
                        self.clock_samples.remove(0);
                    }
                    self.clock_samples
                        .push((estimate.rtt_ms, estimate.offset_ms));
                }
            }
            Command::SimulateDrift { drift_ms } => {
                if drift_ms.unsigned_abs() > 10_000 {
                    error = Some("INVALID_DRIFT");
                } else if self.sample.loaded && !self.snapshot_required {
                    let target_ms = (self.sample.position_ms as i64 - drift_ms)
                        .clamp(0, self.sample.duration_ms as i64)
                        as u64;
                    let c = self.sync.observe(Observation {
                        target_ms,
                        actual_ms: self.sample.position_ms,
                        now_ms: now,
                        playing: self.sample.playing,
                        buffering: self.sample.buffering || self.sample.seeking,
                        clock_trusted: self.trusted(now),
                        supports_rate: self.caps.playback_rate,
                        nominal_rate: 1.0,
                    });
                    match c {
                        Correction::None => {}
                        Correction::SetRate(rate) => effects.push(self.effect("rate", rate)),
                        Correction::Seek(p) => effects.push(self.effect("seek", p as f64)),
                    }
                }
            }
            Command::Observe { target_ms } => {
                if target_ms > self.sample.duration_ms {
                    error = Some("SEEK_OUT_OF_RANGE");
                } else if self.sample.loaded && !self.snapshot_required {
                    let c = self.sync.observe(Observation {
                        target_ms,
                        actual_ms: self.sample.position_ms,
                        now_ms: now,
                        playing: self.sample.playing,
                        buffering: self.sample.buffering || self.sample.seeking,
                        clock_trusted: self.trusted(now),
                        supports_rate: self.caps.playback_rate,
                        nominal_rate: 1.0,
                    });
                    match c {
                        Correction::None => {}
                        Correction::SetRate(rate) => effects.push(self.effect("rate", rate)),
                        Correction::Seek(p) => effects.push(self.effect("seek", p as f64)),
                    }
                }
            }
            Command::SchedulePlay { deadline_ms } => {
                if deadline_ms < now || deadline_ms > now.saturating_add(2_000) {
                    error = Some("INVALID_DEADLINE");
                } else if !self.trusted(now) || self.snapshot_required || !self.sample.loaded {
                    error = Some("RECOVERY_REQUIRED");
                } else {
                    self.deadline = Some(deadline_ms);
                }
            }
            Command::CancelHash => {
                if let Some(job) = &self.job {
                    job.cancel.store(true, Ordering::Release);
                }
            }
        }
        self.reply(now, effects, error)
    }
    #[cfg(unix)]
    fn start_hash(&mut self, fd: i32) -> Result<(), &'static str> {
        use std::os::fd::{FromRawFd, OwnedFd};
        if self.suspended {
            return Err("SUSPENDED");
        }
        if self
            .job
            .as_ref()
            .is_some_and(|j| j.worker.as_ref().is_some_and(|w| !w.is_finished()))
        {
            return Err("HASH_BUSY");
        }
        // dup verifies the descriptor; Rust owns only the duplicate. Reject pipes:
        // blocking provider reads cannot be cooperatively cancelled by this spike.
        let duplicate = unsafe { libc::dup(fd) };
        if duplicate < 0 {
            return Err("INVALID_HANDLE");
        }
        let owned = unsafe { OwnedFd::from_raw_fd(duplicate) };
        let mut file = std::fs::File::from(owned);
        let before = file.metadata().map_err(|_| "READ_FAILED")?;
        if !before.is_file() {
            return Err("UNSUPPORTED_SOURCE");
        }
        use std::io::Seek;
        file.rewind().map_err(|_| "UNSUPPORTED_SOURCE")?;
        let cancel = Arc::new(AtomicBool::new(false));
        let status = Arc::new(Mutex::new(HashStatus {
            state: "running",
            size_bytes: before.len(),
            ..HashStatus::default()
        }));
        let token = cancel.clone();
        let published = status.clone();
        let worker = std::thread::spawn(move || {
            use std::os::unix::fs::MetadataExt;
            let started = Instant::now();
            let result = cine_local_media::hash_reader(&mut file, Some(before.len()), |p| {
                published.lock().unwrap().read_bytes = p.read_bytes;
                !token.load(Ordering::Acquire)
            });
            let changed = file
                .metadata()
                .map(|m| {
                    m.len() != before.len()
                        || m.mtime() != before.mtime()
                        || m.mtime_nsec() != before.mtime_nsec()
                        || m.ctime() != before.ctime()
                        || m.ctime_nsec() != before.ctime_nsec()
                })
                .unwrap_or(true);
            let mut s = published.lock().unwrap();
            s.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            match result {
                Ok(identity) if !changed && !token.load(Ordering::Acquire) => {
                    s.state = "complete";
                    s.size_bytes = identity.size_bytes;
                    s.digest = Some(identity.sha256.iter().map(|b| format!("{b:02x}")).collect());
                }
                Err(cine_local_media::MediaError::Cancelled) => s.state = "cancelled",
                _ if token.load(Ordering::Acquire) => s.state = "cancelled",
                _ if changed => s.state = "modified",
                _ => s.state = "read_failed",
            }
        });
        self.job = Some(HashJob {
            cancel,
            status,
            worker: Some(worker),
        });
        Ok(())
    }
}
struct Instance {
    started: Instant,
    app: Application,
}
static REGISTRY: OnceLock<Mutex<HashMap<u64, Instance>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<HashMap<u64, Instance>> {
    REGISTRY.get_or_init(Mutex::default)
}
#[unsafe(no_mangle)]
pub extern "C" fn cine_bridge_create() -> u64 {
    let mut instances = registry().lock().unwrap();
    if instances.len() >= 16 {
        return 0;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    instances.insert(
        id,
        Instance {
            started: Instant::now(),
            app: Application::default(),
        },
    );
    id
}
#[unsafe(no_mangle)]
pub extern "C" fn cine_bridge_destroy(handle: u64) -> i32 {
    // Remove under the registry lock, join outside it.
    let instance = registry().lock().unwrap().remove(&handle);
    if let Some(instance) = instance {
        drop(instance);
        0
    } else {
        -1
    }
}
/// # Safety
/// Input/output must be valid disjoint memory regions of the stated sizes.
/// Output capacity >=64 KiB is required before dispatch, so retries never mutate twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cine_bridge_call(
    handle: u64,
    input: *const u8,
    length: usize,
    output: *mut u8,
    capacity: usize,
) -> i32 {
    if input.is_null() || output.is_null() || length > LIMIT || capacity < LIMIT {
        return -2;
    }
    let bytes = unsafe { std::slice::from_raw_parts(input, length) };
    let mut instances = registry().lock().unwrap();
    let Some(instance) = instances.get_mut(&handle) else {
        return -1;
    };
    let now = instance.started.elapsed().as_millis() as u64;
    let reply = instance.app.dispatch(bytes, now);
    let serialized = serde_json::to_vec(&reply).unwrap();
    if serialized.len() > capacity {
        return -3;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(serialized.as_ptr(), output, serialized.len());
    }
    serialized.len() as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn cine_bridge_hash_fd(handle: u64, fd: i32) -> i32 {
    let mut instances = registry().lock().unwrap();
    let Some(instance) = instances.get_mut(&handle) else {
        return -1;
    };
    #[cfg(unix)]
    {
        match instance.app.start_hash(fd) {
            Ok(()) => 0,
            Err("HASH_BUSY") => -4,
            Err("SUSPENDED") => -5,
            Err(_) => -6,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (instance, fd);
        -6
    }
}

#[cfg(all(test, unix))]
mod fd_tests {
    use super::*;
    use std::os::fd::AsRawFd;
    #[test]
    fn cancellation_of_fd_worker_never_publishes_identity_and_closes_duplicate() {
        let path = std::env::temp_dir().join(format!("cine-bridge-cancel-{}", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(8 * 1024 * 1024).unwrap();
        drop(file);
        let file = std::fs::File::open(&path).unwrap();
        let mut app = Application::default();
        app.start_hash(file.as_raw_fd()).unwrap();
        app.job
            .as_ref()
            .unwrap()
            .cancel
            .store(true, Ordering::Release);
        let mut job = app.job.take().unwrap();
        job.worker.take().unwrap().join().unwrap();
        assert_eq!(job.status.lock().unwrap().state, "cancelled");
        assert!(job.status.lock().unwrap().digest.is_none());
        // Original borrowed FD remains owned and usable by the native adapter.
        assert_eq!(file.metadata().unwrap().len(), 8 * 1024 * 1024);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn fd_worker_hashes_known_bytes_and_rejects_non_regular_source() {
        let path = std::env::temp_dir().join(format!("cine-bridge-vector-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut app = Application::default();
        app.start_hash(file.as_raw_fd()).unwrap();
        let mut job = app.job.take().unwrap();
        job.worker.take().unwrap().join().unwrap();
        assert_eq!(
            job.status.lock().unwrap().digest.as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(job.status.lock().unwrap().state, "complete");
        let (socket, _) = std::os::unix::net::UnixStream::pair().unwrap();
        assert_eq!(
            app.start_hash(socket.as_raw_fd()),
            Err("UNSUPPORTED_SOURCE")
        );
        std::fs::remove_file(path).unwrap();
    }
}
