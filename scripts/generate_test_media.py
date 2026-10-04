"""Generate only synthetic media; outputs remain ignored by Git. Requires ffmpeg."""
import argparse
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("test-media"))
    parser.add_argument("--long-seconds", type=int, default=620)
    args = parser.parse_args()
    if args.long_seconds < 610:
        parser.error("long corpus must allow a real 600-second run")
    args.output.mkdir(parents=True, exist_ok=True)
    encoders = subprocess.check_output(["ffmpeg", "-hide_banner", "-encoders"], text=True)
    if "libx264" not in encoders or " aac " not in encoders:
        raise SystemExit("This corpus requires local libx264 and AAC encoders; no silent codec substitution.")
    cases = [("normal.mp4", 30, 30, 30, False),
             ("long-gop.mp4", 30, 30, 300, False),
             ("variable-framerate.mkv", 30, 30, 90, True),
             ("audio-video.mp4", 30, 30, 60, False),
             ("long-duration.mp4", args.long_seconds, 15, 150, False)]
    manifest = []
    for name, duration, fps, gop, vfr in cases:
        size = "160x90" if duration > 30 else "320x180"
        command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                   "-f", "lavfi", "-i", f"testsrc2=size={size}:rate={fps}",
                   "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
                   "-t", str(duration), "-c:v", "libx264", "-preset", "ultrafast",
                   "-threads", "2", "-pix_fmt", "yuv420p", "-g", str(gop),
                   "-keyint_min", str(gop), "-sc_threshold", "0",
                   "-c:a", "aac", "-b:a", "96k"]
        if vfr:
            command += ["-vf", "select='if(lt(mod(n,60),30),not(mod(n,2)),1)'", "-fps_mode", "vfr"]
        command.append(str(args.output / name))
        subprocess.run(command, check=True)
        probe = json.loads(subprocess.check_output([
            "ffprobe", "-v", "error", "-show_entries",
            "format=duration,size:stream=codec_type,codec_name,width,height,r_frame_rate,avg_frame_rate,sample_rate",
            "-of", "json", str(args.output / name)], text=True))
        frames = json.loads(subprocess.check_output([
            "ffprobe", "-v", "error", "-select_streams", "v:0", "-read_intervals", "0%+4",
            "-show_entries", "frame=best_effort_timestamp_time", "-of", "json",
            str(args.output / name)], text=True))["frames"]
        times = [float(f["best_effort_timestamp_time"]) for f in frames
                 if "best_effort_timestamp_time" in f]
        deltas = sorted({round((b-a)*1000, 3) for a, b in zip(times, times[1:])})
        manifest.append({"name": name, "gop_frames": gop, "variable_frame_rate": vfr,
                         "frame_delta_ms_first_4s": deltas, **probe})
        print(f"Generated {name}", flush=True)
    (args.output / "empty.mp4").write_bytes(b"")
    (args.output / "corrupt.mp4").write_bytes(b"\x00\x00\x00\x20ftypisom" + b"\xff" * 512)
    (args.output / "unsupported.bin").write_bytes(b"Cine Virtual synthetic non-media fixture\n" * 10)
    (args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
