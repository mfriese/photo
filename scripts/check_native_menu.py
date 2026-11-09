#!/usr/bin/env python3
"""Exercise the native AppKit menu in an isolated, self-closing eframe test window."""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    if sys.platform != "darwin":
        raise SystemExit("This native menu check requires macOS/AppKit.")
    subprocess.run(["cargo", "build", "--example", "native-menu-check", "--locked", "--offline"],
                   cwd=ROOT, check=True)
    subprocess.run([str(ROOT / "target/debug/examples/native-menu-check")],
                   cwd=ROOT, check=True, timeout=30)


if __name__ == "__main__":
    main()
