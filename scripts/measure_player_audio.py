"""Measure only a generated 440 Hz tone on a temporary isolated Pulse sink.

No microphones/default desktop monitor are opened. Requires pactl, ffmpeg and
an already-built cine-player-spike. Stdlib only; not a perceptual quality test.
"""
import array
import json
import math
import subprocess
import time
import wave
from pathlib import Path


def main():
    runs = Path("experiments/01-player-crossplatform/runs")
    runs.mkdir(parents=True, exist_ok=True)
    sink = "cine_spike_b_audio"
    module = subprocess.check_output(["pactl", "load-module", "module-null-sink",
                                      f"sink_name={sink}"], text=True).strip()
    recorder = player = None
    try:
        wav = Path("test-media/audio-capture.wav")
        recorder = subprocess.Popen(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y",
                                     "-f", "pulse", "-i", f"{sink}.monitor", "-ac", "1",
                                     "-ar", "48000", "-c:a", "pcm_s16le", str(wav)], stdin=subprocess.PIPE)
        record_start = time.monotonic()
        time.sleep(0.5)
        player = subprocess.Popen(["target/debug/cine-player-spike", "test-media/audio-video.mp4",
                                   "--audio", "--audio-device", f"pulse/{sink}", "--manual"],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  text=True)
        while True:
            line = player.stdout.readline()
            if line.startswith("Commands:"):
                break
            if not line:
                raise RuntimeError("player failed before readiness")
        player.stdin.write("play\n")
        player.stdin.flush()
        time.sleep(0.7)
        stages = []
        for rate in [0.98, 1.00, 1.02, 0.95, 1.05, 1.00]:
            player.stdin.write(f"rate {rate}\n")
            player.stdin.flush()
            switch = time.monotonic() - record_start
            time.sleep(2.7)
            stages.append({"rate": rate, "window_start_seconds": switch + 0.7,
                           "window_end_seconds": switch + 2.3})
        player.stdin.write("pause\nquit\n")
        player.stdin.flush()
        stdout, stderr = player.communicate(timeout=10)
        if player.returncode or "error " in stdout:
            raise RuntimeError(f"player audio controls failed: {stderr}")
        recorder.communicate(input=b"q\n", timeout=10)
        if recorder.returncode:
            raise RuntimeError("audio recorder failed")
        with wave.open(str(wav), "rb") as source:
            assert source.getnchannels() == 1 and source.getsampwidth() == 2
            sample_rate = source.getframerate()
            pcm = array.array("h", source.readframes(source.getnframes()))
        for stage in stages:
            chunk = pcm[int(stage["window_start_seconds"]*sample_rate):int(stage["window_end_seconds"]*sample_rate)]
            if len(chunk) < sample_rate:
                raise RuntimeError("audio capture shorter than measurement window")
            crossings = sum(a <= 0 < b for a, b in zip(chunk, chunk[1:]))
            stage["frequency_hz_zero_crossing"] = crossings / (len(chunk)/sample_rate)
            stage["rms_normalized"] = math.sqrt(sum((v/32768)**2 for v in chunk)/len(chunk))
            windows = [chunk[i:i+480] for i in range(0, len(chunk)-480, 480)]
            stage["silent_10ms_windows"] = sum(sum(v*v for v in w)/len(w) < 100**2 for w in windows)
            stage["total_10ms_windows"] = len(windows)
        result = {"schema_version": 1, "test": "isolated synthetic audio",
                  "unix_timestamp_seconds": int(time.time()), "input_frequency_hz": 440,
                  "sample_rate": sample_rate, "pitch_correction": True,
                  "method": "zero crossings/RMS over steady windows; excludes transitions",
                  "limitations": "No listening test; does not prove music/speech quality, transition artifacts or mobile audio",
                  "measurements": stages}
        (runs/"audio.json").write_text(json.dumps(result, indent=2)+"\n")
        print(json.dumps(result, indent=2))
    finally:
        for process in [player, recorder]:
            if process and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        subprocess.run(["pactl", "unload-module", module], check=True)


if __name__ == "__main__":
    main()
