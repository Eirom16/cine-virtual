//! Experimental local-file libmpv adapter. All calls and event polling belong
//! to one owner thread. No callbacks, Tokio, network or UI types cross this API.
mod ffi;
pub mod measurements;

use cine_core::player::{Player, PlayerError, PlayerErrorCode};
use std::ffi::{CStr, CString, c_int, c_void};
use std::marker::PhantomData;
use std::path::Path;
use std::ptr::NonNull;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    LibraryUnavailable,
    IncompatibleApi,
    Destroyed,
    NotLoaded,
    FileNotFound,
    EmptyMedia,
    InvalidPath,
    LoadFailed,
    SeekOutOfRange,
    SeekPending,
    UnsupportedRate,
    BackendFailure,
    Timeout,
    EventOverflow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterError {
    pub code: ErrorCode,
    pub backend_code: Option<i32>,
}
impl AdapterError {
    fn new(code: ErrorCode) -> Self {
        Self {
            code,
            backend_code: None,
        }
    }
    fn backend(code: ErrorCode, backend: i32) -> Self {
        Self {
            code,
            backend_code: Some(backend),
        }
    }
}
impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.code)
    }
}
impl std::error::Error for AdapterError {}
impl From<AdapterError> for PlayerError {
    fn from(error: AdapterError) -> Self {
        let code = match error.code {
            ErrorCode::NotLoaded => PlayerErrorCode::NotLoaded,
            ErrorCode::UnsupportedRate => PlayerErrorCode::UnsupportedRate,
            ErrorCode::SeekOutOfRange | ErrorCode::SeekPending => PlayerErrorCode::SeekFailed,
            _ => PlayerErrorCode::BackendFailure,
        };
        Self {
            code,
            message: format!("libmpv adapter: {:?}", error.code),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub visible: bool,
    /// External Render API; never creates a native video window.
    pub embedded: bool,
    pub audio: bool,
    pub hardware_decode: bool,
    /// SDK device name, e.g. pulse/cine_spike_b. Never persisted in results.
    pub audio_device: Option<String>,
    pub gpu_context: Option<String>,
    /// Explicit diagnostic opt-in; upstream logs can include the local path.
    pub diagnostic_logs: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Loaded,
    Playing,
    Paused,
    SeekStarted,
    SeekCompleted { target_ms: u64 },
    PlaybackRestart,
    Buffering(bool),
    EndOfFile,
    Error(AdapterError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Empty,
    Loading,
    Ready,
    Ended,
    Failed,
    Destroyed,
}

pub struct MpvPlayer {
    api: ffi::Api,
    handle: Option<NonNull<c_void>>,
    state: State,
    pending_seek: Option<(u64, bool)>,
    // The C API is thread-safe in general, but this wrapper deliberately has
    // one owner, including destruction. Surface integration needs a new review.
    _owner_thread: PhantomData<Rc<()>>,
}
impl MpvPlayer {
    pub fn new(config: Config) -> Result<Self, AdapterError> {
        let api = ffi::Api::load().map_err(|_| AdapterError::new(ErrorCode::LibraryUnavailable))?;
        // Pure SDK query; no handle required.
        let version = unsafe { (api.version)() };
        if version >> 16 != 2 {
            return Err(AdapterError::new(ErrorCode::IncompatibleApi));
        }
        // SDK returns an independently owned handle or NULL on failure.
        let handle = NonNull::new(unsafe { (api.create)() })
            .ok_or(AdapterError::new(ErrorCode::BackendFailure))?;
        let mut player = Self {
            api,
            handle: Some(handle),
            state: State::Empty,
            pending_seek: None,
            _owner_thread: PhantomData,
        };
        let options = [
            ("config", "no"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            (
                "terminal",
                if config.diagnostic_logs { "yes" } else { "no" },
            ),
            (
                "msg-level",
                if config.diagnostic_logs {
                    "all=v"
                } else {
                    "all=no"
                },
            ),
            ("input-default-bindings", "no"),
            ("input-terminal", "no"),
            ("pause", "yes"),
            ("idle", "yes"),
            ("keep-open", "no"),
            ("audio-pitch-correction", "yes"),
            ("hwdec", if config.hardware_decode { "auto" } else { "no" }),
            ("vd-lavc-threads", "2"),
            ("ad-lavc-threads", "1"),
            (
                "vo",
                if config.embedded {
                    "libmpv"
                } else if config.visible {
                    "gpu"
                } else {
                    "null"
                },
            ),
            ("title", "Cine Virtual - Spike B"),
        ];
        for (name, value) in options {
            player.option(name, value)?;
        }
        if !config.audio {
            player.option("ao", "null")?;
        }
        if let Some(context) = config.gpu_context {
            player.option("gpu-context", &context)?;
        }
        if let Some(device) = config.audio_device {
            player.option("audio-device", &device)?;
        }
        // The handle and all option strings are valid throughout each call.
        let rc = unsafe { (player.api.initialize)(handle.as_ptr()) };
        player.check(rc, ErrorCode::BackendFailure)?;
        for (id, name) in [(1, "pause"), (2, "paused-for-cache")] {
            let name = CString::new(name).expect("static property");
            // FLAG observations carry c_int values; events are copied before
            // the next wait_event call invalidates their memory.
            let rc = unsafe { (player.api.observe)(handle.as_ptr(), id, name.as_ptr(), 3) };
            player.check(rc, ErrorCode::BackendFailure)?;
        }
        Ok(player)
    }
    /// Borrowed only by a native presenter holding a RealPlayer lifetime lease.
    /// It may call mpv_render_* on a separate context, never playback APIs.
    pub fn presentation_handle(&self) -> usize {
        self.handle.map_or(0, |h| h.as_ptr() as usize)
    }
    fn handle(&self) -> Result<*mut c_void, AdapterError> {
        self.handle
            .map(NonNull::as_ptr)
            .ok_or(AdapterError::new(ErrorCode::Destroyed))
    }
    fn check(&self, rc: c_int, code: ErrorCode) -> Result<(), AdapterError> {
        if rc < 0 {
            Err(AdapterError::backend(code, rc))
        } else {
            Ok(())
        }
    }
    fn option(&mut self, name: &str, value: &str) -> Result<(), AdapterError> {
        let name = CString::new(name).map_err(|_| AdapterError::new(ErrorCode::InvalidPath))?;
        let value = CString::new(value).map_err(|_| AdapterError::new(ErrorCode::InvalidPath))?;
        // Both C strings outlive the synchronous option call.
        let rc = unsafe { (self.api.option)(self.handle()?, name.as_ptr(), value.as_ptr()) };
        self.check(rc, ErrorCode::BackendFailure)
    }
    fn command(&mut self, args: &[&str]) -> Result<(), AdapterError> {
        let strings: Vec<_> = args
            .iter()
            .map(|s| CString::new(*s).map_err(|_| AdapterError::new(ErrorCode::InvalidPath)))
            .collect::<Result<_, _>>()?;
        let mut pointers: Vec<_> = strings.iter().map(|s| s.as_ptr()).collect();
        pointers.push(std::ptr::null());
        // Null-terminated argument array and all C strings remain live.
        let rc = unsafe { (self.api.command)(self.handle()?, pointers.as_ptr()) };
        self.check(rc, ErrorCode::BackendFailure)
    }
    fn number(&self, name: &str) -> Result<f64, AdapterError> {
        let name = CString::new(name).expect("property has no NUL");
        let mut value: f64 = 0.0;
        // MPV_FORMAT_DOUBLE writes exactly one f64 into this output buffer.
        let rc = unsafe {
            (self.api.get)(
                self.handle()?,
                name.as_ptr(),
                5,
                (&mut value as *mut f64).cast(),
            )
        };
        self.check(rc, ErrorCode::NotLoaded)?;
        if !value.is_finite() || value < 0.0 {
            return Err(AdapterError::new(ErrorCode::BackendFailure));
        }
        Ok(value)
    }
    pub fn property_text(&self, name: &str) -> Result<Option<String>, AdapterError> {
        let name = CString::new(name).map_err(|_| AdapterError::new(ErrorCode::InvalidPath))?;
        // SDK allocates this NUL-terminated string. Copy it and release with
        // mpv_free; never retain event/property pointers across SDK calls.
        let ptr = unsafe { (self.api.string)(self.handle()?, name.as_ptr()) };
        if ptr.is_null() {
            return Ok(None);
        }
        let value = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.api.free)(ptr.cast()) };
        Ok(Some(value))
    }
    fn set_number(&mut self, name: &str, mut value: f64) -> Result<(), AdapterError> {
        let name = CString::new(name).expect("static property");
        // DOUBLE input buffer is borrowed only for this synchronous call.
        let rc = unsafe {
            (self.api.set)(
                self.handle()?,
                name.as_ptr(),
                5,
                (&mut value as *mut f64).cast(),
            )
        };
        self.check(rc, ErrorCode::BackendFailure)
    }
    fn set_paused(&mut self, paused: bool) -> Result<(), AdapterError> {
        self.ensure_loaded()?;
        let mut flag: c_int = i32::from(paused);
        // FLAG input is a C int, not a Rust bool.
        let rc = unsafe {
            (self.api.set)(
                self.handle()?,
                c"pause".as_ptr(),
                3,
                (&mut flag as *mut c_int).cast(),
            )
        };
        self.check(rc, ErrorCode::BackendFailure)
    }
    fn ensure_loaded(&self) -> Result<(), AdapterError> {
        self.handle()?;
        if self.state != State::Ready {
            Err(AdapterError::new(ErrorCode::NotLoaded))
        } else {
            Ok(())
        }
    }
    /// Local path is a device handle, never a MediaDescriptor or room field.
    /// Dispatch is asynchronous; readiness is signalled by Loaded + restart.
    pub fn load(&mut self, path: &Path) -> Result<(), AdapterError> {
        self.handle()?;
        if self.state == State::Loading || self.pending_seek.is_some() {
            return Err(AdapterError::new(ErrorCode::SeekPending));
        }
        let metadata = path
            .metadata()
            .map_err(|_| AdapterError::new(ErrorCode::FileNotFound))?;
        if !metadata.is_file() {
            return Err(AdapterError::new(ErrorCode::InvalidPath));
        }
        if metadata.len() == 0 {
            return Err(AdapterError::new(ErrorCode::EmptyMedia));
        }
        let path = path
            .canonicalize()
            .map_err(|_| AdapterError::new(ErrorCode::FileNotFound))?;
        let path = path
            .to_str()
            .ok_or(AdapterError::new(ErrorCode::InvalidPath))?;
        // Stop and drain the previous file before a new load. Otherwise its
        // delayed END_FILE can incorrectly mark the new load as failed.
        self.command(&["stop"])?;
        let until = Instant::now() + Duration::from_secs(3);
        while self.property_text("idle-active")?.as_deref() != Some("yes") {
            self.poll(Duration::from_millis(10))?;
            if Instant::now() > until {
                return Err(AdapterError::new(ErrorCode::Timeout));
            }
        }
        while !self.poll(Duration::ZERO)?.is_empty() {}
        self.command(&["set", "pause", "yes"])?;
        self.state = State::Loading;
        if let Err(error) = self.command(&["loadfile", path, "replace"]) {
            self.state = State::Failed;
            return Err(error);
        }
        Ok(())
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn paused(&self) -> Result<bool, AdapterError> {
        Ok(self.property_text("pause")?.as_deref() == Some("yes"))
    }
    pub fn seek_to(&mut self, target_ms: u64) -> Result<(), AdapterError> {
        self.ensure_loaded()?;
        if self.pending_seek.is_some() {
            return Err(AdapterError::new(ErrorCode::SeekPending));
        }
        let duration = (self.number("duration")? * 1000.0).round() as u64;
        validate_seek(target_ms, duration)?;
        self.command(&[
            "seek",
            &format!("{:.6}", target_ms as f64 / 1000.0),
            "absolute+exact",
        ])?;
        self.pending_seek = Some((target_ms, false));
        Ok(())
    }
    pub fn rate(&mut self, rate: f64) -> Result<(), AdapterError> {
        self.ensure_loaded()?;
        validate_rate(rate)?;
        self.set_number("speed", rate)
    }
    /// Process a bounded batch. The first call may wait, subsequent calls drain.
    pub fn poll(&mut self, wait: Duration) -> Result<Vec<Event>, AdapterError> {
        let handle = self.handle()?;
        let mut events = Vec::new();
        for index in 0..256 {
            // Event storage belongs to mpv and is valid until the next wait.
            // Every value exposed to callers is copied before that next call.
            let raw = unsafe {
                (self.api.event)(handle, if index == 0 { wait.as_secs_f64() } else { 0.0 })
            };
            if raw.is_null() {
                return Err(AdapterError::new(ErrorCode::BackendFailure));
            }
            let raw = unsafe { &*raw };
            match raw.id {
                0 => break,
                1 => return Err(AdapterError::new(ErrorCode::Destroyed)),
                7 => {
                    if raw.data.is_null() {
                        return Err(AdapterError::new(ErrorCode::BackendFailure));
                    }
                    // END_FILE payload matches upstream struct layout, API >=2.
                    let end = unsafe { &*raw.data.cast::<ffi::EndFile>() };
                    self.pending_seek = None;
                    match end.reason {
                        0 => {
                            self.state = State::Ended;
                            events.push(Event::EndOfFile);
                        }
                        4 => {
                            let code = if self.state == State::Loading {
                                ErrorCode::LoadFailed
                            } else {
                                ErrorCode::BackendFailure
                            };
                            self.state = State::Failed;
                            events.push(Event::Error(AdapterError::backend(code, end.error)));
                        }
                        _ => {
                            self.state = State::Empty;
                        }
                    }
                }
                8 => {
                    self.state = State::Ready;
                    events.push(Event::Loaded);
                }
                20 => {
                    if let Some((_, started)) = &mut self.pending_seek {
                        *started = true;
                    }
                    events.push(Event::SeekStarted);
                }
                21 => {
                    if let Some((target_ms, true)) = self.pending_seek {
                        self.pending_seek = None;
                        events.push(Event::SeekCompleted { target_ms });
                    } else {
                        events.push(Event::PlaybackRestart);
                    }
                }
                22 => {
                    if raw.data.is_null() {
                        continue;
                    }
                    // Observed FLAG data is c_int; unavailable properties have
                    // format NONE and must never be dereferenced.
                    let property = unsafe { &*raw.data.cast::<ffi::Property>() };
                    if property.format != 3 || property.data.is_null() {
                        continue;
                    }
                    let value = unsafe { *property.data.cast::<c_int>() } != 0;
                    match raw.userdata {
                        1 => events.push(if value { Event::Paused } else { Event::Playing }),
                        2 => events.push(Event::Buffering(value)),
                        _ => {}
                    }
                }
                24 => return Err(AdapterError::new(ErrorCode::EventOverflow)),
                _ => {}
            }
        }
        Ok(events)
    }
    pub fn screenshot(&mut self, path: &Path) -> Result<(), AdapterError> {
        self.ensure_loaded()?;
        self.command(&[
            "screenshot-to-file",
            path.to_str()
                .ok_or(AdapterError::new(ErrorCode::InvalidPath))?,
            "video",
        ])
    }
    pub fn destroy(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Sole owner; no callbacks or other clients remain. Terminate waits
            // for the playback core/decoder threads before the library unloads.
            unsafe { (self.api.destroy)(handle.as_ptr()) };
        }
        self.pending_seek = None;
        self.state = State::Destroyed;
    }
}
impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.destroy();
    }
}
impl Player for MpvPlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        self.set_paused(false).map_err(Into::into)
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        self.set_paused(true).map_err(Into::into)
    }
    fn seek(&mut self, position_ms: u64) -> Result<(), PlayerError> {
        self.seek_to(position_ms).map_err(Into::into)
    }
    fn position(&self) -> Result<u64, PlayerError> {
        self.ensure_loaded()?;
        Ok((self.number("time-pos")? * 1000.0).round() as u64)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        self.ensure_loaded()?;
        Ok((self.number("duration")? * 1000.0).round() as u64)
    }
    fn supports_playback_rate(&self) -> bool {
        self.handle.is_some()
    }
    fn set_playback_rate(&mut self, rate: f64) -> Result<(), PlayerError> {
        self.rate(rate).map_err(Into::into)
    }
}
fn validate_seek(target: u64, duration: u64) -> Result<(), AdapterError> {
    if target > duration {
        Err(AdapterError::new(ErrorCode::SeekOutOfRange))
    } else {
        Ok(())
    }
}
fn validate_rate(rate: f64) -> Result<(), AdapterError> {
    if !rate.is_finite() || !(0.5..=2.0).contains(&rate) {
        Err(AdapterError::new(ErrorCode::UnsupportedRate))
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_rates_never_reach_sdk() {
        for rate in [f64::NAN, f64::INFINITY, 0.0, -1.0, 2.01] {
            assert_eq!(
                validate_rate(rate).unwrap_err().code,
                ErrorCode::UnsupportedRate
            );
        }
        for rate in [0.95, 0.98, 1.0, 1.02, 1.05] {
            assert!(validate_rate(rate).is_ok());
        }
    }
    #[test]
    fn seek_bounds_and_domain_error_mapping() {
        assert!(validate_seek(20_000, 20_000).is_ok());
        let error = validate_seek(20_001, 20_000).unwrap_err();
        assert_eq!(PlayerError::from(error).code, PlayerErrorCode::SeekFailed);
        assert_eq!(
            PlayerError::from(AdapterError::new(ErrorCode::NotLoaded)).code,
            PlayerErrorCode::NotLoaded
        );
    }
}
