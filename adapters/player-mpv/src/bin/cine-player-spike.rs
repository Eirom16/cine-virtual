use cine_core::player::Player;
use cine_player_mpv::{Config, MpvPlayer, measurements};
use serde_json::json;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "cine-player-spike [PATH] [--visible] [--audio] [--hwdec] [--manual] [--long SECONDS] [--output JSON] [--audio-device SDK_NAME] [--gpu-context CONTEXT] [--diagnostic-logs]\nWithout --manual it measures load/play/pause/seek/rates/sync/EOF. Default: headless, null audio, software decode."
        );
        return Ok(());
    }
    let option = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
    };
    let path = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from);
    let visible = args.iter().any(|a| a == "--visible");
    let config = Config {
        visible,
        embedded: false,
        audio: visible || args.iter().any(|a| a == "--audio"),
        hardware_decode: args.iter().any(|a| a == "--hwdec"),
        audio_device: option("--audio-device").cloned(),
        gpu_context: option("--gpu-context").cloned(),
        diagnostic_logs: args.iter().any(|a| a == "--diagnostic-logs"),
    };
    let mut player = MpvPlayer::new(config)?;
    if args.iter().any(|a| a == "--manual") {
        if let Some(path) = path {
            measurements::load_ready(&mut player, &path)?;
        }
        manual(&mut player)?;
    } else {
        let path = path.ok_or("provide a local corpus path, or --manual")?;
        let result = if let Some(seconds) = option("--long") {
            measurements::prolonged(&mut player, &path, seconds.parse()?)?
        } else {
            measurements::automatic(&mut player, &path, visible)?
        };
        let document = json!({"experiment":"Spike B","schema_version":1,"platform":std::env::consts::OS,
            "candidate":"libmpv","unix_timestamp_seconds":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs(),"result":result});
        let output = serde_json::to_string_pretty(&document)?;
        if let Some(file) = option("--output") {
            std::fs::write(file, format!("{output}\n"))?;
        }
        println!("{output}");
    }
    player.destroy();
    Ok(())
}
fn manual(player: &mut MpvPlayer) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Commands: load <path>, play, pause, seek <ms>, position, duration, rate <value>, state, quit"
    );
    let (tx, rx) = mpsc::sync_channel(16);
    let reader = std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            let quit = line.trim() == "quit";
            if tx.send(line).is_err() || quit {
                break;
            }
        }
    });
    loop {
        for event in player.poll(Duration::from_millis(10))? {
            println!("event {event:?}");
        }
        match rx.try_recv() {
            Ok(line) => {
                let (command, arg) = line.trim().split_once(' ').unwrap_or((line.trim(), ""));
                if command == "quit" {
                    break;
                }
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    match command {
                        "load" => {
                            let elapsed = measurements::load_ready(player, Path::new(arg))?;
                            println!("loaded_ms={elapsed:.3}");
                        }
                        "play" => player.play()?,
                        "pause" => player.pause()?,
                        "seek" => player.seek(arg.parse()?)?,
                        "rate" => player.set_playback_rate(arg.parse()?)?,
                        "position" => println!("position_ms={}", player.position()?),
                        "duration" => println!("duration_ms={}", player.duration()?),
                        "state" => println!(
                            "state={:?} paused={:?} position={:?}",
                            player.state(),
                            player.paused(),
                            player.position()
                        ),
                        _ => println!("unknown command"),
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    println!("error {error}");
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
    // On quit/EOF the stdin reader has already left its loop; no detached task.
    reader.join().map_err(|_| "stdin reader failed")?;
    Ok(())
}
