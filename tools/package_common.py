"""Shared license collection for local development packages."""
import json
import os
from pathlib import Path
import shutil
import subprocess


def copy_notices(root, destination):
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("LICENSE", "THIRD_PARTY_NOTICES.md", "README.md"):
        shutil.copy2(root / name, destination / name)
    env = dict(os.environ)
    env.setdefault("CARGO_HOME", str(root / ".cache/cargo"))
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--locked", "--all-features", "--format-version", "1"], cwd=root, env=env))
    for package in metadata["packages"]:
        if not package["source"]:
            continue
        source = Path(package["manifest_path"]).parent
        candidates = set()
        for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "COPYRIGHT*", "NOTICE*"):
            candidates.update(source.glob(pattern))
        if package["license_file"]:
            candidates.add(source / package["license_file"])
        output = destination / "third-party" / f"{package['name']}-{package['version']}"
        for candidate in candidates:
            if candidate.is_file():
                output.mkdir(parents=True, exist_ok=True)
                shutil.copy2(candidate, output / candidate.name)
