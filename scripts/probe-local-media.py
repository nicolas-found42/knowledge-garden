#!/usr/bin/env python3
"""Probe local image and speech processing with isolated repository fixtures."""

import base64
import json
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PHOTO = ROOT / "docs/research/expanded-exploration/01-source-interpretation/experiments/media-lab/fruits.jpg"
AUDIO = ROOT / "src-tauri/tests/fixtures/frozen-two-speaker-uncertain-correction.wav"
WHISPER = ROOT / "src-tauri/resources/audio/whisper-cli"
WHISPER_MODEL = ROOT / "src-tauri/resources/audio/ggml-large-v3-turbo.bin"


def run(*args):
    subprocess.run(args, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def vision(image, prompt):
    body = {
        "model": "qwen2.5vl:7b",
        "prompt": prompt,
        "images": [base64.b64encode(image.read_bytes()).decode()],
        "stream": False,
        "options": {"num_predict": 150, "temperature": 0},
    }
    request = urllib.request.Request(
        "http://127.0.0.1:11434/api/generate",
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    started = time.monotonic()
    with urllib.request.urlopen(request, timeout=300) as response:
        result = json.load(response)
    result.pop("context", None)
    result["wall_seconds"] = time.monotonic() - started
    return result


def main():
    with tempfile.TemporaryDirectory(prefix="garden-media-probe-") as temp:
        directory = Path(temp)
        video, frame, sound = (directory / name for name in ("video.mp4", "frame.png", "audio.wav"))
        run("ffmpeg", "-v", "error", "-loop", "1", "-i", str(PHOTO), "-i", str(AUDIO),
            "-c:v", "libx264", "-tune", "stillimage", "-pix_fmt", "yuv420p", "-c:a", "aac",
            "-shortest", "-y", str(video))
        run("ffmpeg", "-v", "error", "-ss", "5", "-i", str(video), "-frames:v", "1", "-y", str(frame))
        run("ffmpeg", "-v", "error", "-i", str(video), "-vn", "-ar", "16000", "-ac", "1", "-y", str(sound))
        run(str(WHISPER), "-m", str(WHISPER_MODEL), "-f", str(sound), "-l", "en", "-oj", "-of", str(directory / "transcript"))
        segments = json.loads((directory / "transcript.json").read_text())["transcription"]
        picture = vision(frame, "Describe the visible items and their colors. Only describe what you can see.")
        transcript = "\n".join(f"{s['timestamps']['from']} to {s['timestamps']['to']}: {s['text']}" for s in segments)
        combined = vision(frame, "The image is a video frame at 00:00:05. The following is an unverified transcript with timestamps. Identify one visible fruit and report the uncertain count, citing the relevant times. Preserve uncertainty.\n" + transcript)
        print(json.dumps({"frame_time_seconds": 5, "segments": segments, "visual": picture, "combined": combined}, indent=2))
        print("INCONCLUSIVE: Inspect visual claims, timestamps, uncertainty, and unsupported relationships manually.", file=sys.stderr)


if __name__ == "__main__":
    main()
