#!/usr/bin/env python3
"""Opt-in HTTPS smoke test for the native model transport; downloads <=3 KiB of metadata."""
import argparse
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
URL = "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/.gitattributes"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--network", action="store_true", help="Allow two HTTPS requests for pinned metadata (never model weights)")
    options = parser.parse_args()
    if not options.network:
        parser.error("Pass --network to explicitly allow the small HTTPS smoke test")
    with tempfile.TemporaryDirectory(prefix="photosuite-model-test-") as folder:
        directory = Path(folder)
        binary = directory / "model-download-test"
        subprocess.run(["clang", "-fobjc-arc", "-Wno-deprecated-declarations", "-I", str(ROOT / "native"), str(ROOT / "tests/native_model_download.m"), str(ROOT / "native/whisper_models.m"), "-framework", "Foundation", "-o", str(binary)], check=True)
        subprocess.run([str(binary), URL, str(directory / "metadata.txt")], check=True, timeout=75)


if __name__ == "__main__":
    main()
