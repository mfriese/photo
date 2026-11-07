#!/usr/bin/env python3
"""Build a self-contained macOS app; install only on explicit invocation."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]
IDENTIFIER = "de.photosuite.PhotoSuite"
APP_NAME = "PhotoSuite.app"


def run(args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)


def build_icon(resources):
    source = ROOT / "assets/app-icon/photosuite.png"
    if not source.is_file():
        raise RuntimeError("App-Icon fehlt: assets/app-icon/photosuite.png")
    with tempfile.TemporaryDirectory(prefix="photosuite-icon-") as temp:
        iconset = Path(temp) / "PhotoSuite.iconset"
        iconset.mkdir()
        for size in [16, 32, 128, 256, 512]:
            for scale in [1, 2]:
                suffix = "@2x" if scale == 2 else ""
                destination = iconset / f"icon_{size}x{size}{suffix}.png"
                run(["/usr/bin/sips", "--resampleHeightWidth", size * scale, size * scale, source, "--out", destination], stdout=subprocess.DEVNULL)
        run(["/usr/bin/iconutil", "--convert", "icns", "--output", resources / "PhotoSuite.icns", iconset])


def exif_distribution():
    tool = shutil.which("exiftool")
    if not tool:
        raise RuntimeError("Zum Paketbau wird eine ExifTool-Perl-Distribution benötigt.")
    tool = Path(tool).resolve()
    # Prefer the original script over Homebrew's path-rewritten wrapper.
    original = tool.parent.parent / "libexec/bin/exiftool"
    if original.is_file():
        tool = original
    for lib in [tool.parent / "lib", tool.parent.parent / "lib", tool.parent.parent / "lib/perl5"]:
        if (lib / "Image/ExifTool.pm").is_file() and (lib / "File/RandomAccess.pm").is_file():
            return tool, lib
    raise RuntimeError("ExifTool-Bibliothek nicht gefunden; vollständige Perl-Distribution verwenden.")


def copy_exif(resources):
    tool, lib = exif_distribution()
    target = resources / "exiftool"
    target.mkdir()
    script = tool.read_text()
    if "/opt/homebrew/" in script or "/usr/local/Cellar/" in script:
        raise RuntimeError("ExifTool-Skript enthält externe Homebrew-Pfade; Originaldistribution benötigt.")
    # Interpreter comes from macOS, not from a versioned Homebrew shebang.
    script = "#!/usr/bin/perl\n" + script.split("\n", 1)[1]
    (target / "exiftool").write_text(script)
    (target / "lib/File").mkdir(parents=True)
    shutil.copytree(lib / "Image", target / "lib/Image")
    for path in (lib / "File").glob("RandomAccess.*"):
        shutil.copy2(path, target / "lib/File" / path.name)
    notices = resources / "Licenses/ExifTool"
    notices.mkdir(parents=True)
    readmes = [tool.parent.parent / "README", tool.parent.parent.parent / "README"]
    for source in readmes:
        if source.is_file():
            shutil.copy2(source, notices / "README")
            break
    for name in ["perlartistic", "perlgpl"]:
        output = run(["/usr/bin/perldoc", "-l", name], capture_output=True, text=True)
        shutil.copyfile(Path(output.stdout.strip()), notices / f"{name}.pod")


def rust_notices(resources, metadata):
    notices = resources / "Licenses/Rust"
    notices.mkdir(parents=True)
    lines = []
    for package in metadata["packages"]:
        if package.get("source") is None:
            continue
        name = f'{package["name"]}-{package["version"]}'
        lines.append(f'{name}: {package.get("license") or "see license files"}')
        folder = Path(package["manifest_path"]).parent
        files = [p for p in folder.iterdir() if p.is_file() and re.match(r"^(LICENSE|LICENCE|COPYING|NOTICE)", p.name, re.I)]
        if package.get("license_file"):
            files.append(folder / package["license_file"])
        for source in set(files):
            if source.is_file():
                destination = notices / name / source.name
                destination.parent.mkdir(exist_ok=True)
                shutil.copy2(source, destination)
    (resources / "Licenses/THIRD_PARTY.txt").write_text("\n".join(lines) + "\n")


def verify_bundle(app):
    with (app / "Contents/Info.plist").open("rb") as file:
        info = plistlib.load(file)
    if info.get("CFBundleIdentifier") != IDENTIFIER:
        raise RuntimeError("Unbekannte App-ID; Paket wird nicht installiert/entfernt.")
    if info.get("CFBundleIconFile") != "PhotoSuite.icns" or not (app / "Contents/Resources/PhotoSuite.icns").is_file():
        raise RuntimeError("App-Icon fehlt oder ist nicht in Info.plist registriert.")
    binary = app / "Contents/MacOS/photosuite"
    libraries = run(["/usr/bin/otool", "-L", binary], capture_output=True, text=True).stdout
    for line in libraries.splitlines()[1:]:
        library = line.strip().split(" (", 1)[0]
        if not library.startswith(("/System/Library/", "/usr/lib/")):
            raise RuntimeError(f"Nicht enthaltene Laufzeitbibliothek: {library}")
    run(["/usr/bin/codesign", "--verify", "--deep", "--strict", app])
    env = dict(os.environ, PATH="/usr/bin:/bin:/usr/sbin:/sbin", PERL5LIB="", PERLLIB="")
    run([binary, "--check-installation"], env=env)


def build():
    if sys.platform != "darwin":
        raise RuntimeError("Das App-Paket kann nur auf macOS gebaut werden.")
    run(["cargo", "build", "--release", "--locked", "--offline", "-p", "photosuite"], cwd=ROOT)
    host = run(["rustc", "-vV"], capture_output=True, text=True).stdout.split("host: ", 1)[1].splitlines()[0]
    metadata = json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", host], cwd=ROOT, capture_output=True, text=True).stdout)
    package = next(p for p in metadata["packages"] if p["name"] == "photosuite")
    target = Path(metadata["target_directory"])
    output = target / "macos"
    output.mkdir(exist_ok=True)
    app = output / APP_NAME
    with tempfile.TemporaryDirectory(prefix=".photosuite-build-", dir=output) as temp:
        stage = Path(temp) / APP_NAME
        contents = stage / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        resources = contents / "Resources"
        resources.mkdir()
        shutil.copy2(target / "release/photosuite", contents / "MacOS/photosuite")
        info = {
            "CFBundleIdentifier": IDENTIFIER,
            "CFBundleName": "PhotoSuite",
            "CFBundleDisplayName": "PhotoSuite",
            "CFBundleExecutable": "photosuite",
            "CFBundleIconFile": "PhotoSuite.icns",
            "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": package["version"],
            "CFBundleVersion": package["version"],
            "CFBundleDevelopmentRegion": "de",
            "NSHighResolutionCapable": True,
            "NSPrincipalClass": "NSApplication",
        }
        with (contents / "Info.plist").open("wb") as file:
            plistlib.dump(info, file)
        (contents / "PkgInfo").write_bytes(b"APPL????")
        build_icon(resources)
        copy_exif(resources)
        rust_notices(resources, metadata)
        shutil.copy2(ROOT / "README.md", resources / "README.md")
        run(["/usr/bin/plutil", "-lint", contents / "Info.plist"])
        run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", stage])
        verify_bundle(stage)
        replace_app(stage, app)
    print(f"App-Paket erstellt: {app}")
    return app


def assert_owned(app):
    if app.is_symlink():
        raise RuntimeError("Symbolische App-Pfade werden nicht ersetzt.")
    with (app / "Contents/Info.plist").open("rb") as file:
        if plistlib.load(file).get("CFBundleIdentifier") != IDENTIFIER:
            raise RuntimeError("Ziel ist keine PhotoSuite-Installation; Abbruch.")


def replace_app(source, target):
    if target.is_symlink():
        raise RuntimeError("Symbolische App-Pfade werden nicht ersetzt.")
    previous = target.with_name(f".PhotoSuite-previous-{uuid.uuid4().hex}.app")
    existed = target.exists()
    if existed:
        assert_owned(target)
        target.rename(previous)
    try:
        source.rename(target)
    except BaseException:
        if existed:
            previous.rename(target)
        raise
    if existed:
        shutil.rmtree(previous)


def ensure_closed(target):
    processes = run(["/bin/ps", "-axo", "comm="], capture_output=True, text=True).stdout
    if str(target / "Contents/MacOS/photosuite") in processes:
        raise RuntimeError("PhotoSuite vor Installation/Deinstallation schließen.")


def install(app, directory, register=True):
    directory = directory.expanduser().absolute()
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / APP_NAME
    ensure_closed(target)
    verify_bundle(app)
    with tempfile.TemporaryDirectory(prefix=".photosuite-install-", dir=directory) as temp:
        stage = Path(temp) / APP_NAME
        shutil.copytree(app, stage, symlinks=False)
        verify_bundle(stage)
        replace_app(stage, target)
    if register:
        run(["/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister", "-f", target])
    print(f"Installiert: {target}")
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["build", "install", "uninstall", "verify", "package"])
    parser.add_argument("--destination", type=Path, default=Path("/Applications"))
    parser.add_argument("--app", type=Path, help="Bereits gebautes App-Paket verwenden")
    args = parser.parse_args()
    if args.action == "build":
        build()
    elif args.action == "install":
        install(args.app or build(), args.destination)
    elif args.action == "package":
        app = args.app or build()
        verify_bundle(app)
        with (app / "Contents/Info.plist").open("rb") as file:
            version = plistlib.load(file)["CFBundleShortVersionString"]
        package = app.parent / f"PhotoSuite-{version}.pkg"
        # Disable Installer relocation so an older copy elsewhere cannot redirect /Applications.
        with tempfile.TemporaryDirectory(prefix=".photosuite-pkg-", dir=app.parent) as temp:
            root = Path(temp) / "payload"
            root.mkdir()
            shutil.copytree(app, root / APP_NAME, symlinks=False)
            components = Path(temp) / "components.plist"
            run(["/usr/bin/pkgbuild", "--analyze", "--root", root, components])
            with components.open("rb") as file:
                configuration = plistlib.load(file)
            for component in configuration:
                component["BundleIsRelocatable"] = False
                component["BundleHasStrictIdentifier"] = True
                component["BundleOverwriteAction"] = "upgrade"
            with components.open("wb") as file:
                plistlib.dump(configuration, file)
            run(["/usr/bin/pkgbuild", "--root", root, "--component-plist", components, "--install-location", "/Applications", "--identifier", IDENTIFIER, "--version", version, package])
        print(f"Installationspaket erstellt: {package}")
    elif args.action == "verify":
        verify_bundle(args.app or ROOT / "target/macos" / APP_NAME)
    else:
        target = args.destination / APP_NAME
        if target.exists():
            assert_owned(target)
            ensure_closed(target)
            shutil.rmtree(target)
            print(f"Entfernt: {target}; Preferences und Fotoquellen bleiben erhalten.")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        print(f"Fehler: {error}", file=sys.stderr)
        sys.exit(1)
