#!/usr/bin/env python3
"""Verify immutable historical format reference bytes; never regenerate manifests."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures/formats"

def main():
    manifests = sorted(ROOT.glob("baseline-*.json"))
    if not manifests:
        raise SystemExit("Missing format baseline")
    for manifest in manifests:
        for name, expected in json.loads(manifest.read_text()).items():
            path = ROOT / name
            if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise SystemExit(f"Historical format fixture changed or missing: {name}")
    print("Historical format fixture bytes OK")

if __name__ == "__main__":
    main()
