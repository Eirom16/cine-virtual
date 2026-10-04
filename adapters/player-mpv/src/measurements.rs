//! Real-time experiment harness. Timers here never enter deterministic Core.
use crate::{AdapterError, ErrorCode, Event, MpvPlayer};
use cine_core::player::Player;
use cine_core::sync::{Correction, Observation, SyncConfig, SyncEngine};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn wait_event(
    player: &mut MpvPlayer,
    wanted: impl Fn(Event) -> bool,
    timeout: Duration,
) -> Result<Event> {
    let deadline = Instant::now() + timeout;
    loop {
        for event in player.poll(Duration::from_millis(5))? {
            if let Event::Error(error) = event {
                return Err(error.into());
            }
            if wanted(event) {
                return Ok(event);
            }
        }
        if Instant::now() >= deadline {
            return Err(AdapterError {
                code: ErrorCode::Timeout,
                backend_code: None,
            }
            .into());
        }
    }
}
pub fn load_ready(player: &mut MpvPlayer, path: &Path) -> Result<f64> {
    let start = Instant::now();
    player.load(path)?;
    // Decoder restart after FILE_LOADED is a stronger barrier than merely
    // accepting loadfile. Still not evidence of a physically presented frame.
    wait_event(
        player,
        |e| e == Event::PlaybackRestart,
        Duration::from_secs(10),
    )?;
    player.duration()?;
    player.position()?;
    Ok(ms(start.elapsed()))
}
fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
fn pump(player: &mut MpvPlayer, duration: Duration) -> Result<()> {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        for event in player.poll(Duration::from_millis(5))? {
            if let Event::Error(error) = event {
                return Err(error.into());
            }
        }
    }
    Ok(())
}
fn pause_measure(player: &mut MpvPlayer) -> Result<Value> {
    let before = player.position()?;
    let start = Instant::now();
    player.pause()?;
    let dispatch = ms(start.elapsed());
    let mut previous = player.position()?;
    let mut stable = 0;
    while stable < 5 {
        pump(player, Duration::from_millis(10))?;
        let position = player.position()?;
        stable = if position == previous { stable + 1 } else { 0 };
        previous = position;
        if start.elapsed() > Duration::from_secs(3) {
            return Err("pause failed to stabilize".into());
        }
    }
    Ok(
        json!({"dispatch_ms":dispatch,"stable_ms":ms(start.elapsed()),
        "before_ms":before,"after_ms":previous,"advance_ms":previous as i64 - before as i64}),
    )
}
fn play_measure(player: &mut MpvPlayer) -> Result<Value> {
    // Drain initial/stale pause notifications before measuring a new command.
    pump(player, Duration::from_millis(10))?;
    let before = player.position()?;
    let start = Instant::now();
    player.play()?;
    let dispatch = ms(start.elapsed());
    wait_event(player, |e| e == Event::Playing, Duration::from_secs(3))?;
    let state_ms = ms(start.elapsed());
    while player.position()? <= before {
        pump(player, Duration::from_millis(5))?;
        if start.elapsed() > Duration::from_secs(3) {
            return Err("play position did not advance".into());
        }
    }
    Ok(json!({"dispatch_ms":dispatch,"playing_event_ms":state_ms,
        "position_advancing_ms":ms(start.elapsed()),"start_position_ms":before}))
}
pub fn seek_measure(player: &mut MpvPlayer, target: u64) -> Result<Value> {
    let start = Instant::now();
    player.seek_to(target)?;
    let dispatch = ms(start.elapsed());
    wait_event(
        player,
        |e| matches!(e, Event::SeekCompleted {target_ms} if target_ms == target),
        Duration::from_secs(5),
    )?;
    let completion = ms(start.elapsed());
    let reported = player.position()?;
    let mut stable_ms = None;
    if player.paused()? {
        let mut previous = reported;
        let mut count = 0;
        while count < 3 {
            pump(player, Duration::from_millis(10))?;
            let position = player.position()?;
            count = if position == previous { count + 1 } else { 0 };
            previous = position;
            if start.elapsed() > Duration::from_secs(5) {
                return Err("seek position unstable".into());
            }
        }
        stable_ms = Some(ms(start.elapsed()));
    }
    Ok(
        json!({"target_ms":target,"dispatch_ms":dispatch,"completion_ms":completion,
        "stable_ms":stable_ms,"reported_ms":reported,"landing_error_ms":reported as i64-target as i64}),
    )
}
fn samples(player: &mut MpvPlayer, seconds: f64, origin: Instant) -> Result<Vec<Value>> {
    let start = Instant::now();
    let mut samples = Vec::new();
    while start.elapsed().as_secs_f64() < seconds {
        pump(player, Duration::from_millis(50))?;
        samples.push(json!({"monotonic_ms":ms(origin.elapsed()),"position_ms":player.position()?}));
    }
    Ok(samples)
}
fn sample_summary(samples: &[Value]) -> Value {
    let mut backsteps = 0;
    let mut repeats = 0;
    let mut residuals = Vec::new();
    for pair in samples.windows(2) {
        let delta =
            pair[1]["position_ms"].as_f64().unwrap() - pair[0]["position_ms"].as_f64().unwrap();
        let elapsed =
            pair[1]["monotonic_ms"].as_f64().unwrap() - pair[0]["monotonic_ms"].as_f64().unwrap();
        if delta < 0.0 {
            backsteps += 1;
        }
        if delta == 0.0 {
            repeats += 1;
        }
        residuals.push((delta - elapsed).abs());
    }
    residuals.sort_by(f64::total_cmp);
    let p95 = residuals
        .get((residuals.len().saturating_sub(1) as f64 * 0.95) as usize)
        .copied();
    json!({"count":samples.len(),"backsteps":backsteps,"repeats":repeats,"delta_residual_abs_p95_ms":p95})
}
fn rate_measure(player: &mut MpvPlayer, rate: f64, origin: Instant) -> Result<Value> {
    player.set_playback_rate(rate)?;
    pump(player, Duration::from_millis(300))?;
    let observed = samples(player, 2.0, origin)?;
    let first = observed.first().ok_or("no position samples")?;
    let last = observed.last().ok_or("no position samples")?;
    let slope = (last["position_ms"].as_f64().unwrap() - first["position_ms"].as_f64().unwrap())
        / (last["monotonic_ms"].as_f64().unwrap() - first["monotonic_ms"].as_f64().unwrap());
    Ok(
        json!({"requested":rate,"sdk_speed":player.property_text("speed")?,
        "observed_position_rate":slope,"sampling":sample_summary(&observed),"samples":observed}),
    )
}
fn sync_trial(player: &mut MpvPlayer, lead: u64, seconds: f64) -> Result<Value> {
    player.set_playback_rate(1.0)?;
    pump(player, Duration::from_millis(300))?;
    let actual = player.position()?;
    let anchor = actual.saturating_sub(lead);
    let origin = Instant::now();
    let mut engine = SyncEngine::new(SyncConfig::default()).map_err(|_| "invalid sync config")?;
    let mut observations = Vec::new();
    let mut corrections = Vec::new();
    while origin.elapsed().as_secs_f64() < seconds {
        pump(player, Duration::from_millis(100))?;
        let now = origin.elapsed().as_millis() as u64;
        let target = anchor + now;
        let actual = player.position()?;
        observations.push(
            json!({"monotonic_ms":now,"target_ms":target,"actual_ms":actual,
            "drift_ms":actual as i64 - target as i64}),
        );
        let correction = engine.observe(Observation {
            target_ms: target,
            actual_ms: actual,
            now_ms: now,
            playing: true,
            buffering: false,
            clock_trusted: true,
            supports_rate: player.supports_playback_rate(),
            nominal_rate: 1.0,
        });
        match correction {
            Correction::None => {}
            Correction::SetRate(rate) => {
                player.set_playback_rate(rate)?;
                corrections.push(json!({"at_ms":now,"type":"rate","rate":rate}));
            }
            Correction::Seek(target) => {
                let measurement = seek_measure(player, target)?;
                corrections.push(json!({"at_ms":now,"type":"seek","measurement":measurement}));
            }
        }
    }
    player.set_playback_rate(1.0)?;
    let final_drift =
        player.position()? as i64 - (anchor + origin.elapsed().as_millis() as u64) as i64;
    Ok(
        json!({"injected_lead_ms":lead,"duration_ms":ms(origin.elapsed()),"final_drift_ms":final_drift,
        "corrections":corrections,"samples":observations}),
    )
}
/// No personal paths or SDK payloads are returned. Media is represented by
/// properties only; the caller records a known corpus basename separately.
pub fn automatic(player: &mut MpvPlayer, path: &Path, visible: bool) -> Result<Value> {
    let origin = Instant::now();
    let load_ms = load_ready(player, path)?;
    let duration = player.duration()?;
    if duration < 25_000 {
        return Err(
            "automatic harness requires at least 25 seconds; use manual for shorter media".into(),
        );
    }
    let properties = media_properties(player)?;
    eprintln!("stage: load ready; beginning controls");
    let initial_pause = pause_measure(player)?;
    let mut seeks = Vec::new();
    seeks.push(seek_measure(player, 5000.min(duration / 4))?);
    let first_play = play_measure(player)?;
    let position_samples = samples(player, 1.5, origin)?;
    let first_pause = pause_measure(player)?;
    seeks.push(seek_measure(player, 20_000.min(duration * 2 / 3))?);
    let second_play = play_measure(player)?;
    pump(player, Duration::from_millis(400))?;
    let second_pause = pause_measure(player)?;
    seeks.push(seek_measure(player, 1000.min(duration / 10))?);
    play_measure(player)?;
    eprintln!("stage: rates");
    let mut rates = Vec::new();
    for rate in [0.98, 1.0, 1.02, 0.95, 1.05, 1.0] {
        rates.push(rate_measure(player, rate, origin)?);
    }
    pause_measure(player)?;
    seeks.push(seek_measure(player, 5000.min(duration / 5))?);
    play_measure(player)?;
    eprintln!("stage: sync soft");
    let soft = sync_trial(player, 120, 9.0)?;
    eprintln!("stage: sync hard");
    let hard = sync_trial(player, 700, 2.0)?;
    pause_measure(player)?;
    seeks.push(seek_measure(player, duration.saturating_sub(800))?);
    if visible {
        player.screenshot(Path::new("test-media/visible-frame.png"))?;
    }
    let final_play = play_measure(player)?;
    eprintln!("stage: EOF");
    let end = Instant::now();
    wait_event(player, |e| e == Event::EndOfFile, Duration::from_secs(5))?;
    let resources = resources();
    Ok(
        json!({"load_ms":load_ms,"duration_ms":duration,"media_properties":properties,
        "play":[first_play,second_play,final_play],"pause":[initial_pause,first_pause,second_pause],
        "seek":seeks,"position_sampling":sample_summary(&position_samples),"position_samples":position_samples,
        "rate":rates,"sync_soft":soft,"sync_hard":hard,"eof_wait_ms":ms(end.elapsed()),
        "resources":resources,"elapsed_ms":ms(origin.elapsed()),"mode":if visible {"visible"} else {"headless"}}),
    )
}
fn media_properties(player: &MpvPlayer) -> Result<Value> {
    let mut values = serde_json::Map::new();
    // Allowlist: never include path, title, device IDs or arbitrary SDK logs.
    for name in [
        "mpv-version",
        "ffmpeg-version",
        "video-codec",
        "audio-codec",
        "video-params/w",
        "video-params/h",
        "container-fps",
        "hwdec-current",
        "current-vo",
        "current-ao",
        "audio-pitch-correction",
    ] {
        values.insert(name.into(), json!(player.property_text(name)?));
    }
    Ok(Value::Object(values))
}
pub fn prolonged(player: &mut MpvPlayer, path: &Path, seconds: u64) -> Result<Value> {
    if !(1..=3600).contains(&seconds) {
        return Err("long run must be between 1 and 3600 real seconds".into());
    }
    let load_ms = load_ready(player, path)?;
    if player.duration()? < (seconds + 2) * 1000 {
        return Err("media too short for requested real-time run".into());
    }
    let properties = media_properties(player)?;
    let play = play_measure(player)?;
    let anchor = player.position()?;
    let start = Instant::now();
    let before = resources();
    let mut engine = SyncEngine::new(SyncConfig::default()).map_err(|_| "invalid sync config")?;
    let mut records = Vec::new();
    let mut corrections = Vec::new();
    let mut minute = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        pump(player, Duration::from_millis(500))?;
        let now = start.elapsed().as_millis() as u64;
        let actual = player.position()?;
        let target = anchor + now;
        records.push(
            json!({"monotonic_ms":now,"position_ms":actual,"drift_ms":actual as i64-target as i64}),
        );
        match engine.observe(Observation {
            target_ms: target,
            actual_ms: actual,
            now_ms: now,
            playing: true,
            buffering: false,
            clock_trusted: true,
            supports_rate: true,
            nominal_rate: 1.0,
        }) {
            Correction::None => {}
            Correction::SetRate(rate) => {
                player.set_playback_rate(rate)?;
                corrections.push(json!({"at_ms":now,"type":"rate","rate":rate}));
            }
            Correction::Seek(target) => {
                corrections.push(
                    json!({"at_ms":now,"type":"seek","measurement":seek_measure(player,target)?}),
                );
            }
        }
        if now / 60_000 > minute {
            minute = now / 60_000;
            eprintln!(
                "prolonged: minute {minute}, drift {} ms",
                actual as i64 - target as i64
            );
        }
    }
    let after = resources();
    let elapsed = start.elapsed().as_secs_f64();
    let tick_hz = std::env::var("CINE_SPIKE_CLOCK_TICKS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0);
    let cpu = match (before["cpu_ticks"].as_f64(), after["cpu_ticks"].as_f64()) {
        (Some(a), Some(b)) => tick_hz.map(|hz| (b - a) / hz / elapsed * 100.0),
        _ => None,
    };
    player.set_playback_rate(1.0)?;
    let pause = pause_measure(player)?;
    let mut drifts: Vec<_> = records
        .iter()
        .map(|v| v["drift_ms"].as_i64().unwrap().unsigned_abs())
        .collect();
    drifts.sort();
    Ok(
        json!({"load_ms":load_ms,"play":play,"pause":pause,"media_properties":properties,
        "actual_elapsed_ms":elapsed*1000.0,"requested_seconds":seconds,"clock_accelerated":false,
        "sampling":sample_summary(&records),"drift_abs_p95_ms":drifts[(drifts.len()-1)*95/100],
        "drift_abs_max_ms":drifts.last(),"corrections":corrections,"samples":records,
        "resources_before":before,"resources_after":after,"cpu_percent_one_core":cpu,
        "cpu_tick_hz":tick_hz}),
    )
}
pub fn resources() -> Value {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        let value = |key: &str| {
            status
                .lines()
                .find(|l| l.starts_with(key))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|n| n.parse::<u64>().ok())
        };
        let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
        let fields: Vec<_> = stat
            .rsplit_once(')')
            .map(|(_, tail)| tail.split_whitespace().collect())
            .unwrap_or_default();
        let ticks = fields
            .get(11)
            .and_then(|v| v.parse::<u64>().ok())
            .zip(fields.get(12).and_then(|v| v.parse::<u64>().ok()))
            .map(|(a, b)| a + b);
        json!({"rss_kib":value("VmRSS:"),"threads":value("Threads:"),"cpu_ticks":ticks,
            "open_fds":std::fs::read_dir("/proc/self/fd").ok().map(|d|d.count())})
    }
    #[cfg(not(target_os = "linux"))]
    json!({"unmeasured":true})
}
