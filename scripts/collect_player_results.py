"""Consolidate actual, reviewed Spike B runs; no media or private paths exported."""
import json
import re
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPERIMENT = ROOT / "experiments/01-player-crossplatform"
RUNS = EXPERIMENT / "runs"


def main():
    names = {"normal": "normal.mp4", "long-gop": "long-gop.mp4",
             "vfr": "variable-framerate.mkv", "visible": "audio-video.mp4", "long": "long-duration.mp4"}
    runs = {}
    for name, media in names.items():
        document = json.loads((RUNS / f"{name}.json").read_text())
        assert document["candidate"] == "libmpv" and document["platform"] == "linux"
        runs[name] = {"media": media, **document}
    audio = json.loads((RUNS / "audio.json").read_text())
    tests = (RUNS / "sdk-tests.txt").read_text()
    assert "4 passed; 0 failed" in tests
    lifecycle = (RUNS / "lifecycle-tests.txt").read_text()
    match = re.search(r"lifecycle_cycles=(\d+) elapsed_ms=(\d+) before=(\{.*?\}) after=(\{.*?\}) checkpoints=(\[.*\])", lifecycle)
    if not match:
        raise SystemExit("Run the 60-cycle lifecycle test with --nocapture and save lifecycle-tests.txt")
    result = {
        "schema_version": 1, "experiment": "Spike B v1", "platform": "Linux/CachyOS/Arch x86_64",
        "candidate": "libmpv", "candidate_version": "0.41.0", "client_api_version": "2.5",
        "ffmpeg_version": "n9.0.2", "collected_unix_timestamp_seconds": int(time.time()),
        "source_base_commit": "5ccce24", "decision": "D: sufficient for provisional Linux vertical slice",
        "media": json.loads((ROOT / "test-media/manifest.json").read_text()),
        "runs": runs, "audio": audio,
        "lifecycle": {"cycles": int(match[1]), "elapsed_ms": int(match[2]),
                      "before": json.loads(match[3]), "after": json.loads(match[4]),
                      "checkpoints_every_20_cycles": json.loads(match[5]),
                      "observed": "Threads/FDs return to baseline. RSS retained; no absence-of-leaks claim."},
        "errors": {"method": "Assertions in real_player::typed_errors_and_destroy, executed on SDK",
                   "cases": {"missing_file": "FileNotFound", "empty_file": "EmptyMedia",
                             "corrupt_file": "LoadFailed", "unsupported_fixture": "LoadFailed",
                             "seek_past_duration": "SeekOutOfRange", "second_pending_seek": "SeekPending",
                             "zero_or_NaN_rate": "UnsupportedRate (adapter range, not an SDK capability failure)",
                             "before_load": "NotLoaded", "after_destroy": "Destroyed/BackendFailure"},
                   "sdk_tests_passed": 4},
        "visible_evidence": {"sdk_window_capture_inspected": True, "window_pixels": [320, 180],
                             "content": "generated testsrc2 color/moving pattern; own window only",
                             "backend": "GPU/X11 EGL on Xwayland", "hardware_reported": "vaapi",
                             "listening_test": False},
        "limitations": ["Position/completion are SDK reports, not physical presentation timestamps",
                        "Rate slope is endpoint delta over about 2 seconds; quantization/latency affect it",
                        "Pause stable_ms includes an intentional 5x10ms observation window",
                        "Delta residual compares position increments to nominal monotonic increments",
                        "Long-run CPU uses 100 Hz, confirmed locally with getconf CLK_TCK",
                        "Long run used the same headless playback logic before audio-output diagnostic fixes",
                        "No Windows/macOS/Android/iOS builds/runtime; no embedded UI surface",
                        "Synthetic 440Hz steady windows do not establish perceptual audio quality",
                        "No benchmark guarantees from a single low-resolution machine/corpus"],
    }
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if re.search(r"/home/|/Users/|invite_token|resume_token", encoded):
        raise SystemExit("Personal path/credential detected; refusing export")
    (EXPERIMENT / "results-linux.json").write_text(encoded)
    print("Consolidated five actual media runs, audio, typed errors and lifecycle; no personal paths.")


if __name__ == "__main__":
    main()
