#!/usr/bin/env python3
"""Download credited stock photos for documentation, isolated from real libraries.

Explicit network operation; the app itself never downloads demo photographs.
Only creates new files, never overwrites a pre-existing photo or preferences file.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import urllib.request

PHOTO_IDS = [1015, 1016, 1018, 1035, 1036, 1037, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 28, 29]
ROOT = Path(__file__).resolve().parents[1]


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "PhotoSuite documentation demo"})
    with urllib.request.urlopen(request, timeout=40) as response:
        return response.read()


def create_photo(job):
    index, photo_id, destination = job
    info = json.loads(fetch(f"https://picsum.photos/id/{photo_id}/info"))
    year, month = [(2025, 10), (2026, 6), (2026, 8), (2026, 9)][min(index // 5, 3)]
    day = index + 1
    folder = destination / str(year) / f"{month:02}"
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"img_{year}{month:02}{day:02}_153000.jpg"
    if not path.exists():
        # Keep the original aspect ratio, enough resolution for the large preview.
        width = 1600
        height = round(width * info["height"] / info["width"])
        data = fetch(f"https://picsum.photos/id/{photo_id}/{width}/{height}")
        with path.open("xb") as output:
            output.write(data)
        tool = shutil.which("exiftool")
        if tool:
            subprocess.run([tool, "-overwrite_original", f"-DateTimeOriginal={year}:{month:02}:{day:02} 15:30:00", str(path)], check=True, stdout=subprocess.DEVNULL)
    return {"file": str(path.relative_to(destination)), "author": info["author"], "source": info["url"], "id": photo_id}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, default=Path("/tmp/PhotoSuite-Demo"))
    args = parser.parse_args()
    if args.destination.is_symlink():
        raise SystemExit("Demo-Ziel darf kein symbolischer Link sein")
    destination = args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with ThreadPoolExecutor(max_workers=6) as pool:
        sources = list(pool.map(create_photo, [(i, photo_id, destination) for i, photo_id in enumerate(PHOTO_IDS)]))
    (destination / "sources.json").write_text(json.dumps(sources, ensure_ascii=False, indent=2) + "\n")
    prefs = destination / "demo-preferences.plist"
    if not prefs.exists():
        with prefs.open("xb") as output:
            plistlib.dump({"version": 1, "last_library": str(destination), "reopen_library": True, "preview_size": 2, "square_previews": True, "write_exif": False}, output)
    print(f"Demo: {destination}\nSeparate Preferences: {prefs}")


if __name__ == "__main__":
    main()
