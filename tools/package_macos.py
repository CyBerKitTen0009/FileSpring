#!/usr/bin/env python3
"""Build an unsigned, local macOS app bundle without changing system settings."""
import argparse
import json
import os
import plistlib
import shutil
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--debug", action="store_true")
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
binary = root / "target" / ("debug" if args.debug else "release") / "filespring-desktop"
if not binary.is_file():
    parser.error(f"Build first: {binary}")
bundle = root / "artifacts" / "FileSpring.app" / "Contents"
(bundle / "MacOS").mkdir(parents=True, exist_ok=True)
shutil.copy2(binary, bundle / "MacOS" / "FileSpring")
resources = bundle / "Resources"
resources.mkdir(exist_ok=True)
for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "README.md"):
    shutil.copy2(root / name, resources / name)
os.environ.setdefault("CARGO_HOME", str(root / ".cache" / "cargo"))
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--locked", "--all-features", "--format-version", "1"], cwd=root))
for package in metadata["packages"]:
    if not package["source"]:
        continue
    source = Path(package["manifest_path"]).parent
    candidates = set()
    for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "COPYRIGHT*", "NOTICE*"):
        candidates.update(source.glob(pattern))
    if package["license_file"]:
        candidates.add(source / package["license_file"])
    destination = resources / "third-party" / f"{package['name']}-{package['version']}"
    for candidate in candidates:
        if candidate.is_file():
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy2(candidate, destination / candidate.name)
with (bundle / "Info.plist").open("wb") as stream:
    plistlib.dump({"CFBundleName": "FileSpring", "CFBundleDisplayName": "FileSpring", "CFBundleIdentifier": "io.filespring.desktop", "CFBundleExecutable": "FileSpring", "CFBundlePackageType": "APPL", "CFBundleShortVersionString": "0.1.0", "CFBundleVersion": "1", "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "11.0"}, stream)
print(bundle.parent)
