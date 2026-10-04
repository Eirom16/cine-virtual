//! Opt-in SDK tests; ordinary workspace tests do not require libmpv/media.
use cine_core::player::{Player, PlayerErrorCode};
use cine_player_mpv::{Config, ErrorCode, MpvPlayer, State, measurements};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-media")
        .join(name)
}
fn create() -> MpvPlayer {
    MpvPlayer::new(Config::default()).expect("install libmpv before SDK tests")
}

#[test]
#[ignore = "requires libmpv and scripts/generate_test_media.py"]
fn decode_seek_play_pause_rate_eof() {
    let mut player = create();
    let result = measurements::automatic(&mut player, &corpus("normal.mp4"), false).unwrap();
    assert_eq!(result["duration_ms"], 30_000);
    assert!(result["position_sampling"]["count"].as_u64().unwrap() > 20);
    assert_eq!(result["position_sampling"]["backsteps"], 0);
    for seek in result["seek"].as_array().unwrap() {
        assert!(seek["landing_error_ms"].as_i64().unwrap().abs() < 100);
        assert!(seek["completion_ms"].as_f64().unwrap() < 1500.0);
    }
    for rate in result["rate"].as_array().unwrap() {
        assert!(
            (rate["observed_position_rate"].as_f64().unwrap()
                - rate["requested"].as_f64().unwrap())
            .abs()
                < 0.04
        );
    }
    // SDK position is frame-quantized. Assert convergence over the final
    // second, allowing one frame beyond Core's deadband, not a lucky instant.
    let fps: f64 = result["media_properties"]["container-fps"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let tolerance =
        cine_core::sync::SyncConfig::default().deadband_ms + (1000.0 / fps).ceil() as u64;
    let mut tail: Vec<_> = result["sync_soft"]["samples"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .take(10)
        .map(|v| v["drift_ms"].as_i64().unwrap().unsigned_abs())
        .collect();
    tail.sort();
    let median = tail[tail.len() / 2];
    println!(
        "sync_soft_tail_median_ms={median} final_ms={} tolerance_ms={tolerance}",
        result["sync_soft"]["final_drift_ms"]
    );
    assert!(
        median <= tolerance,
        "soft correction did not converge: median={median} tolerance={tolerance}"
    );
    assert!(
        result["sync_soft"]["corrections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["type"] == "rate")
    );
    assert!(
        result["sync_hard"]["corrections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["type"] == "seek")
    );
    assert!(
        result["sync_hard"]["final_drift_ms"]
            .as_i64()
            .unwrap()
            .abs()
            < 100
    );
    assert_eq!(player.state(), State::Ended);
}
#[test]
#[ignore = "requires libmpv and scripts/generate_test_media.py"]
fn typed_errors_and_destroy() {
    let mut player = create();
    assert_eq!(player.play().unwrap_err().code, PlayerErrorCode::NotLoaded);
    assert_eq!(
        player.load(&corpus("not-present.mp4")).unwrap_err().code,
        ErrorCode::FileNotFound
    );
    assert_eq!(
        player.load(&corpus("empty.mp4")).unwrap_err().code,
        ErrorCode::EmptyMedia
    );
    for file in ["corrupt.mp4", "unsupported.bin"] {
        let error = measurements::load_ready(&mut player, &corpus(file)).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<cine_player_mpv::AdapterError>()
                .unwrap()
                .code,
            ErrorCode::LoadFailed
        );
        assert_eq!(player.state(), State::Failed);
    }
    measurements::load_ready(&mut player, &corpus("normal.mp4")).unwrap();
    assert_eq!(
        player.seek_to(30_001).unwrap_err().code,
        ErrorCode::SeekOutOfRange
    );
    assert_eq!(
        player.rate(f64::NAN).unwrap_err().code,
        ErrorCode::UnsupportedRate
    );
    assert_eq!(
        player.rate(0.0).unwrap_err().code,
        ErrorCode::UnsupportedRate
    );
    player.seek_to(5000).unwrap();
    assert_eq!(
        player.seek_to(7000).unwrap_err().code,
        ErrorCode::SeekPending
    );
    player.destroy();
    player.destroy();
    assert_eq!(player.state(), State::Destroyed);
    assert_eq!(
        player.poll(Duration::ZERO).unwrap_err().code,
        ErrorCode::Destroyed
    );
    assert_eq!(
        player.load(&corpus("normal.mp4")).unwrap_err().code,
        ErrorCode::Destroyed
    );
    assert_eq!(
        player.play().unwrap_err().code,
        PlayerErrorCode::BackendFailure
    );
}
#[test]
#[ignore = "requires libmpv and scripts/generate_test_media.py"]
fn replacement_load_preserves_new_generation() {
    let mut player = create();
    measurements::load_ready(&mut player, &corpus("normal.mp4")).unwrap();
    player.play().unwrap();
    measurements::load_ready(&mut player, &corpus("long-duration.mp4")).unwrap();
    assert_eq!(player.duration().unwrap(), 620_000);
    assert_eq!(player.state(), State::Ready);
    assert!(player.paused().unwrap());
    assert!(player.position().unwrap() < 100);
}
#[test]
#[ignore = "requires libmpv and scripts/generate_test_media.py"]
fn repeated_lifecycle_releases_threads_and_files() {
    // Warm SDK process-global caches before comparing ownership resources.
    {
        let mut player = create();
        measurements::load_ready(&mut player, &corpus("normal.mp4")).unwrap();
    }
    let before = measurements::resources();
    let start = Instant::now();
    let mut checkpoints = Vec::new();
    for cycle in 1..=60 {
        let mut player = create();
        measurements::load_ready(&mut player, &corpus("normal.mp4")).unwrap();
        player.play().unwrap();
        player.poll(Duration::from_millis(20)).unwrap();
        player.destroy();
        assert_eq!(player.state(), State::Destroyed);
        if cycle % 20 == 0 {
            checkpoints.push(measurements::resources());
        }
    }
    let after = measurements::resources();
    #[cfg(target_os = "linux")]
    {
        assert!(after["threads"].as_u64().unwrap() <= before["threads"].as_u64().unwrap() + 1);
        assert!(after["open_fds"].as_u64().unwrap() <= before["open_fds"].as_u64().unwrap() + 1);
        for entry in std::fs::read_dir("/proc/self/fd").unwrap().flatten() {
            if let Ok(target) = std::fs::read_link(entry.path()) {
                assert!(
                    !target.ends_with("test-media/normal.mp4"),
                    "media FD retained after destroy"
                );
            }
        }
    }
    println!(
        "lifecycle_cycles=60 elapsed_ms={} before={} after={} checkpoints={}",
        start.elapsed().as_millis(),
        before,
        after,
        serde_json::json!(checkpoints)
    );
}
