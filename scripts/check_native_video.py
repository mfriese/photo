#!/usr/bin/env python3
"""Exercise the native video transport with the repository's small test fixture."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    with tempfile.TemporaryDirectory(prefix="photosuite-video-test-") as folder:
        binary = Path(folder) / "native-video-test"
        subprocess.run([
            "clang", "-fobjc-arc", "-Wno-deprecated-declarations",
            str(ROOT / "tests/native_playback.m"),
            "-framework", "AVFoundation", "-framework", "AVKit",
            "-framework", "AppKit", "-framework", "Foundation",
            "-framework", "CoreMedia", "-framework", "CoreGraphics",
            "-o", str(binary),
        ], check=True)
        subprocess.run([str(binary), str(ROOT / "tests/fixtures/storyboard.mp4")],
                       check=True, timeout=20)


if __name__ == "__main__":
    main()
