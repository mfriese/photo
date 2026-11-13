#!/usr/bin/env python3
"""Verify recorder finalization without microphone access."""
from pathlib import Path
import subprocess
import tempfile
import wave
import math
import struct

ROOT = Path(__file__).resolve().parents[1]


def main():
    with tempfile.TemporaryDirectory(prefix="photosuite-audio-test-") as folder:
        directory = Path(folder)
        files = []
        for seconds in [1, 3]:
            path = directory / f"{seconds}.wav"
            with wave.open(str(path), "wb") as output:
                output.setnchannels(1)
                output.setsampwidth(2)
                output.setframerate(8000)
                output.writeframes(b"".join(struct.pack("<h",int(10000*math.sin(2*math.pi*(220 if seconds==1 else 440)*i/8000))) for i in range(8000*seconds)))
            files.append(path)
        binary = directory / "native-audio-test"
        subprocess.run(["clang", "-fobjc-arc", "-Wno-deprecated-declarations", str(ROOT / "tests/native_recorder.m"), str(ROOT / "native/comment_edit.m"), "-framework", "AVFoundation", "-framework", "AppKit", "-framework", "Foundation", "-framework", "CoreMedia", "-o", str(binary)], check=True)
        subprocess.run([str(binary), str(files[0]),str(files[1])], check=True, timeout=40)


if __name__ == "__main__":
    main()
