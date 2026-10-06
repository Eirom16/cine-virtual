//! Application boundary: one SDK owner thread and a bounded command proxy.
use crate::fake_player::FakePlayer;
use cine_core::player::{Player, PlayerError, PlayerErrorCode};
use cine_player_mpv::{Config, Event, MpvPlayer, measurements};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Default)]
pub struct ControlMark {
    pub sequence: u64,
    pub deadline_server_ms: u64,
    pub offset_ms: f64,
    pub target_ms: u64,
    pub reason: &'static str,
    pub media_revision: u64,
    pub received_at_ms: u64,
    pub deadline_local_ms: f64,
    pub wake_at_ms: u64,
}
#[derive(Clone, Default)]
pub struct PlayerView {
    pub position_ms: u64,
    pub sampled_at_ms: u64,
    pub duration_ms: u64,
    pub playing: bool,
    pub rate: f64,
    pub ready: bool,
    pub seeking: bool,
    pub buffering: bool,
    pub failed: bool,
}
impl Player for PlayerView {
    fn position(&self) -> Result<u64, PlayerError> {
        Ok(self.position_ms)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        Ok(self.duration_ms)
    }
    fn supports_playback_rate(&self) -> bool {
        true
    }
    fn play(&mut self) -> Result<(), PlayerError> {
        Err(failure("Read-only player view"))
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        Err(failure("Read-only player view"))
    }
    fn seek(&mut self, _: u64) -> Result<(), PlayerError> {
        Err(failure("Read-only player view"))
    }
    fn set_playback_rate(&mut self, _: f64) -> Result<(), PlayerError> {
        Err(failure("Read-only player view"))
    }
}
pub trait ApplicationPlayer: Player {
    fn tick(&mut self, now: u64);
    fn configure_duration(&mut self, duration: u64);
    fn view(&self) -> PlayerView;
    fn mark(&mut self, _: ControlMark) {}
    /// Optional bounded diagnostic sink; no SDK or synchronization policy.
    fn diagnostic(&self, _: serde_json::Value) {}
    fn asynchronous(&self) -> bool {
        false
    }
}
impl ApplicationPlayer for FakePlayer {
    fn tick(&mut self, now: u64) {
        self.set_time(now)
    }
    fn configure_duration(&mut self, d: u64) {
        self.duration_ms = d
    }
    fn view(&self) -> PlayerView {
        PlayerView {
            position_ms: self.position().unwrap_or(0),
            duration_ms: self.duration_ms,
            playing: self.playing,
            rate: self.rate,
            ready: true,
            ..Default::default()
        }
    }
}
fn failure(message: &str) -> PlayerError {
    PlayerError {
        code: PlayerErrorCode::BackendFailure,
        message: message.into(),
    }
}
enum Action {
    Load(PathBuf),
    Play,
    Pause,
    Seek(u64),
    Rate(f64),
    Stop,
}
struct Command {
    action: Action,
    mark: ControlMark,
    reply: mpsc::SyncSender<Result<u64, PlayerError>>,
}
struct Owner {
    sender: mpsc::SyncSender<Command>,
    view: Arc<Mutex<PlayerView>>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        let (reply, _) = mpsc::sync_channel(1);
        let _ = self.sender.send(Command {
            action: Action::Stop,
            mark: ControlMark::default(),
            reply,
        });
        if let Some(join) = self.join.lock().unwrap().take() {
            let _ = join.join();
        }
    }
}
#[derive(Clone)]
pub struct RealPlayer {
    owner: Arc<Owner>,
    mark: ControlMark,
}
impl RealPlayer {
    pub fn new(boot: Instant, visible: bool) -> Result<Self, PlayerError> {
        let (tx, rx) = mpsc::sync_channel::<Command>(32);
        let (startup, start) = mpsc::sync_channel(1);
        let view = Arc::new(Mutex::new(PlayerView {
            rate: 1.0,
            ..Default::default()
        }));
        let published = view.clone();
        let join = thread::Builder::new()
            .name("cine-mpv-owner".into())
            .spawn(move || run_owner(boot, visible, rx, published, startup))
            .map_err(|_| failure("PLAYER_THREAD_FAILED"))?;
        match start.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(())) => Ok(Self {
                owner: Arc::new(Owner {
                    sender: tx,
                    view,
                    join: Mutex::new(Some(join)),
                }),
                mark: ControlMark::default(),
            }),
            _ => {
                drop(tx);
                let _ = join.join();
                Err(failure("PLAYER_INITIALIZATION_FAILED"))
            }
        }
    }
    fn send(&self, action: Action) -> Result<u64, PlayerError> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.owner
            .sender
            .try_send(Command {
                action,
                mark: self.mark,
                reply: tx,
            })
            .map_err(|_| failure("PLAYER_QUEUE_FULL_OR_CLOSED"))?;
        rx.recv_timeout(Duration::from_secs(12))
            .map_err(|_| failure("PLAYER_TIMEOUT"))?
    }
    pub fn load(&self, path: PathBuf) -> Result<u64, PlayerError> {
        self.send(Action::Load(path))
    }
    fn snapshot(&self) -> PlayerView {
        self.owner.view.lock().unwrap().clone()
    }
}
impl Player for RealPlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        self.send(Action::Play).map(|_| ())
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        self.send(Action::Pause).map(|_| ())
    }
    fn seek(&mut self, p: u64) -> Result<(), PlayerError> {
        self.send(Action::Seek(p)).map(|_| ())
    }
    fn position(&self) -> Result<u64, PlayerError> {
        Ok(self.snapshot().position_ms)
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        Ok(self.snapshot().duration_ms)
    }
    fn supports_playback_rate(&self) -> bool {
        true
    }
    fn set_playback_rate(&mut self, r: f64) -> Result<(), PlayerError> {
        self.send(Action::Rate(r)).map(|_| ())
    }
}
impl ApplicationPlayer for RealPlayer {
    fn tick(&mut self, _: u64) {}
    fn configure_duration(&mut self, _: u64) {}
    fn view(&self) -> PlayerView {
        self.snapshot()
    }
    fn mark(&mut self, mark: ControlMark) {
        self.mark = mark;
    }
    fn asynchronous(&self) -> bool {
        true
    }
}
pub enum BackendPlayer {
    Fake(FakePlayer),
    Mpv(RealPlayer),
}
impl BackendPlayer {
    pub fn new(kind: &str, boot: Instant, visible: bool) -> Result<Self, PlayerError> {
        match kind {
            "fake" => Ok(Self::Fake(FakePlayer::default())),
            "mpv" => Ok(Self::Mpv(RealPlayer::new(boot, visible)?)),
            _ => Err(failure("UNKNOWN_PLAYER_BACKEND")),
        }
    }
    pub fn real(&self) -> Option<RealPlayer> {
        if let Self::Mpv(p) = self {
            Some(p.clone())
        } else {
            None
        }
    }
}
macro_rules! delegate {
    ($s:ident,$p:ident,$body:expr) => {
        match $s {
            BackendPlayer::Fake($p) => $body,
            BackendPlayer::Mpv($p) => $body,
        }
    };
}
impl Player for BackendPlayer {
    fn play(&mut self) -> Result<(), PlayerError> {
        delegate!(self, p, p.play())
    }
    fn pause(&mut self) -> Result<(), PlayerError> {
        delegate!(self, p, p.pause())
    }
    fn seek(&mut self, n: u64) -> Result<(), PlayerError> {
        delegate!(self, p, p.seek(n))
    }
    fn position(&self) -> Result<u64, PlayerError> {
        delegate!(self, p, p.position())
    }
    fn duration(&self) -> Result<u64, PlayerError> {
        delegate!(self, p, p.duration())
    }
    fn supports_playback_rate(&self) -> bool {
        delegate!(self, p, p.supports_playback_rate())
    }
    fn set_playback_rate(&mut self, r: f64) -> Result<(), PlayerError> {
        delegate!(self, p, p.set_playback_rate(r))
    }
}
impl ApplicationPlayer for BackendPlayer {
    fn tick(&mut self, t: u64) {
        delegate!(self, p, p.tick(t))
    }
    fn configure_duration(&mut self, d: u64) {
        delegate!(self, p, p.configure_duration(d))
    }
    fn view(&self) -> PlayerView {
        delegate!(self, p, p.view())
    }
    fn mark(&mut self, m: ControlMark) {
        delegate!(self, p, p.mark(m))
    }
    fn asynchronous(&self) -> bool {
        delegate!(self, p, p.asynchronous())
    }
}

fn run_owner(
    boot: Instant,
    visible: bool,
    rx: mpsc::Receiver<Command>,
    published: Arc<Mutex<PlayerView>>,
    startup: mpsc::SyncSender<Result<(), PlayerError>>,
) {
    tracing::info!(event="player_resources",stage="before_create",resources=%measurements::resources());
    let mut player = match MpvPlayer::new(Config {
        visible,
        audio: visible,
        ..Default::default()
    }) {
        Ok(p) => {
            let _ = startup.send(Ok(()));
            p
        }
        Err(e) => {
            let _ = startup.send(Err(PlayerError::from(e)));
            return;
        }
    };
    tracing::info!(event="player_resources",stage="after_create",resources=%measurements::resources());
    let now = || boot.elapsed().as_millis() as u64;
    let mut mark = ControlMark::default();
    let mut play_start = None;
    let mut seek_start = None;
    let mut paused_since = None;
    let mut pause_reported = true;
    loop {
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(command) => {
                if matches!(command.action, Action::Stop) {
                    break;
                }
                mark = command.mark;
                let action_kind = match &command.action {
                    Action::Load(_) => "load",
                    Action::Play => "play",
                    Action::Pause => "pause",
                    Action::Seek(_) => "seek",
                    Action::Rate(_) => "rate",
                    Action::Stop => "stop",
                };
                tracing::info!(
                    event = "player_dispatch",
                    kind = action_kind,
                    sequence = mark.sequence,
                    reason = mark.reason,
                    at_ms = now(),
                    deadline_server_ms = mark.deadline_server_ms,
                    actual_server_ms = now() as f64 + mark.offset_ms,
                    target_ms = mark.target_ms
                );
                let result = match command.action {
                    Action::Load(path) => {
                        published.lock().unwrap().ready = false;
                        let r = measurements::load_ready(&mut player, &path)
                            .map_err(|_| failure("PLAYER_LOAD_FAILED"));
                        match r {
                            Ok(_) => {
                                let d = player.duration();
                                if let Ok(d) = d {
                                    *published.lock().unwrap() = PlayerView {
                                        duration_ms: d,
                                        rate: 1.0,
                                        ready: true,
                                        ..Default::default()
                                    };
                                }
                                tracing::info!(event="player_resources",stage="after_load",resources=%measurements::resources());
                                d
                            }
                            Err(e) => Err(e),
                        }
                    }
                    Action::Play => {
                        play_start = Some((now(), player.position().unwrap_or(0)));
                        player.play().map(|_| 0)
                    }
                    Action::Pause => {
                        pause_reported = false;
                        paused_since = None;
                        player.pause().map(|_| 0)
                    }
                    Action::Seek(target) => {
                        seek_start = Some(now());
                        let r = player.seek(target).map(|_| 0);
                        if r.is_ok() {
                            published.lock().unwrap().seeking = true;
                        }
                        r
                    }
                    Action::Rate(rate) => {
                        let r = player.set_playback_rate(rate).map(|_| 0);
                        if r.is_ok() {
                            published.lock().unwrap().rate = rate;
                        }
                        r
                    }
                    Action::Stop => unreachable!(),
                };
                if result
                    .as_ref()
                    .is_err_and(|e| e.code == PlayerErrorCode::BackendFailure)
                {
                    published.lock().unwrap().failed = true;
                }
                let _ = command.reply.send(result);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        match player.poll(Duration::ZERO) {
            Ok(events) => {
                for event in events {
                    let kind = match event {
                        Event::Playing => "playing",
                        Event::Paused => "paused",
                        Event::SeekCompleted { .. } => {
                            published.lock().unwrap().seeking = false;
                            "seek_completed"
                        }
                        Event::Buffering(value) => {
                            published.lock().unwrap().buffering = value;
                            "buffering"
                        }
                        Event::Error(_) => {
                            let mut s = published.lock().unwrap();
                            s.failed = true;
                            s.ready = false;
                            "error"
                        }
                        Event::EndOfFile => {
                            published.lock().unwrap().ready = false;
                            "end_of_file"
                        }
                        _ => continue,
                    };
                    tracing::info!(
                        event = "player_event",
                        kind,
                        sequence = mark.sequence,
                        reason = mark.reason,
                        at_ms = now(),
                        deadline_server_ms = mark.deadline_server_ms,
                        actual_server_ms = now() as f64 + mark.offset_ms,
                        target_ms = mark.target_ms,
                        position_ms = player.position().ok(),
                        latency_ms = if kind == "seek_completed" {
                            seek_start.take().map(|t| now().saturating_sub(t))
                        } else {
                            None
                        }
                    );
                }
            }
            Err(_) => {
                published.lock().unwrap().failed = true;
            }
        }
        if let (Ok(position), Ok(paused)) = (player.position(), player.paused()) {
            let mut s = published.lock().unwrap();
            s.position_ms = position;
            s.sampled_at_ms = now();
            s.playing = !paused;
            if let Some((start, baseline)) = play_start
                && !paused
                && position > baseline
            {
                tracing::info!(
                    event = "player_event",
                    kind = "first_position_advance",
                    sequence = mark.sequence,
                    reason = mark.reason,
                    at_ms = now(),
                    latency_ms = now().saturating_sub(start),
                    position_ms = position
                );
                play_start = None;
            }
            if paused && !s.seeking && !pause_reported {
                let (since, previous) = paused_since.get_or_insert((now(), position));
                if *previous != position {
                    *since = now();
                    *previous = position;
                }
                if now().saturating_sub(*since) >= 50 {
                    tracing::info!(
                        event = "player_event",
                        kind = "pause_stable",
                        sequence = mark.sequence,
                        at_ms = now(),
                        position_ms = position,
                        target_ms = mark.target_ms,
                        error_ms = position as i64 - mark.target_ms as i64
                    );
                    pause_reported = true;
                }
            }
        }
    }
    player.destroy();
    *published.lock().unwrap() = PlayerView::default();
    tracing::info!(event="player_resources",stage="after_destroy",resources=%measurements::resources());
}
