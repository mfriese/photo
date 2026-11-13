#!/usr/bin/env python3
"""Verify progressive macOS playback and in-place timeline corrections using silent WAVs."""
from pathlib import Path
import re
import subprocess
import tempfile
import wave

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
                output.writeframes(b"\0\0" * 8000 * seconds)
            files.append(path)
        colors = re.findall(r"pub const (\w+): Color32 = Color32::from_rgb\(([^)]+)\);", (ROOT / "src/theme.rs").read_text())
        (directory / "theme_palette.h").write_text("\n".join(f"#define PS_THEME_{name} PSThemeColor({rgb})" for name, rgb in colors))
        binary = directory / "native-audio-test"
        subprocess.run(["clang", "-I", str(directory), "-fobjc-arc", "-Wno-deprecated-declarations", str(ROOT / "tests/native_audio.m"), str(ROOT / "native/context_photo_view.m"), str(ROOT / "native/audio_transport.m"), str(ROOT / "native/audio_timeline_view.m"), "-framework", "AVFoundation", "-framework", "AppKit", "-framework", "Foundation", "-framework", "CoreMedia", "-o", str(binary)], check=True)
        subprocess.run([str(binary), *(str(path) for path in files)], check=True, timeout=25)


if __name__ == "__main__":
    main()
