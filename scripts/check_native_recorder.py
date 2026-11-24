#!/usr/bin/env python3
"""Verify recorder finalization without microphone access."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    with tempfile.TemporaryDirectory(prefix="photosuite-audio-test-") as folder:
        directory = Path(folder)
        binary = directory / "native-audio-test"
        subprocess.run(["clang", "-fobjc-arc", "-Wno-deprecated-declarations", str(ROOT / "tests/native_recorder.m"), str(ROOT / "native/comment_edit.m"), "-framework", "AVFoundation", "-framework", "AppKit", "-framework", "Foundation", "-framework", "CoreMedia", "-o", str(binary)], check=True)
        subprocess.run([
            str(binary),
            str(ROOT / "tests/fixtures/recorder-220.m4a"),
            str(ROOT / "tests/fixtures/recorder-440.m4a"),
        ], check=True, timeout=40)


if __name__ == "__main__":
    main()
