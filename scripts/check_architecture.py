#!/usr/bin/env python3
"""Check approved core dependencies using Cargo's actual package metadata."""
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
CORE_DEPENDENCIES = {
    "photosuite-domain": {"chrono"},
    "photosuite-application": {"photosuite-domain"},
}


def violations(metadata):
    packages = {package["name"]: package for package in metadata["packages"]}
    errors = []
    for name, allowed in CORE_DEPENDENCIES.items():
        if name not in packages:
            errors.append(f"Missing core package: {name}")
            continue
        for dependency in packages[name]["dependencies"]:
            if dependency["name"] not in allowed:
                errors.append(f"Unapproved core dependency: {name} -> {dependency['name']}")
    return errors


def main():
    output = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked", "--offline"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    errors = violations(json.loads(output.stdout))
    if errors:
        raise SystemExit("\n".join(errors))
    print("Architecture OK: domain -> chrono; application -> domain; no GUI/infrastructure dependencies in core.")


if __name__ == "__main__":
    main()
