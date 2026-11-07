#!/usr/bin/env python3
"""Package already built Windows x64 PE executables; never relabel Mac binaries."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import zipfile

from package_common import copy_notices
from windows_pe import inspect_pe

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--bin-dir", type=Path, default=Path("target/x86_64-pc-windows-msvc/release"))
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
binary_dir = args.bin_dir.resolve()
version = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
name = f"FileSpring-{version}-windows-x64-portable"
artifacts = root / "artifacts"
artifacts.mkdir(exist_ok=True)
try:
    audit = {"FileSpring.exe": inspect_pe(binary_dir / "filespring-desktop.exe", 2), "FileSpring-cli.exe": inspect_pe(binary_dir / "filespring.exe", 3)}
except (OSError, ValueError) as error:
    parser.error(str(error))
with tempfile.TemporaryDirectory(prefix=".windows-package-", dir=artifacts) as temporary:
    folder = Path(temporary) / name
    folder.mkdir()
    shutil.copy2(binary_dir / "filespring-desktop.exe", folder / "FileSpring.exe")
    shutil.copy2(binary_dir / "filespring.exe", folder / "FileSpring-cli.exe")
    for filename in ("Verify-Windows.cmd", "Launch-Demo.cmd"):
        # CRLF and ASCII keep the launchers compatible with a stock cmd.exe.
        text = (root / "packaging/windows" / filename).read_text(encoding="ascii")
        (folder / filename).write_bytes(text.replace("\r\n", "\n").replace("\n", "\r\n").encode("ascii"))
    shutil.copy2(root / "docs/WINDOWS.md", folder / "README-Windows.md")
    copy_notices(root, folder)
    subprocess.run([sys.executable, str(root / "tools/make_demo.py"), "--output", str(folder / "demo")], check=True, stdout=subprocess.DEVNULL)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    metadata = {"version": version, "source_commit": revision, "source_dirty": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root)), "target": "x86_64-pc-windows-msvc", "signed": False, "runtime_tested": False, "executables": audit}
    (folder / "BUILD-INFO.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    hashes = [f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(folder).as_posix()}" for p in sorted(folder.rglob("*")) if p.is_file()]
    (folder / "SHA256SUMS.txt").write_text("\n".join(hashes) + "\n", encoding="utf-8")
    output = artifacts / f"{name}.zip"
    temporary_zip = Path(temporary) / output.name
    with zipfile.ZipFile(temporary_zip, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9, strict_timestamps=False) as archive:
        for path in sorted(folder.rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(folder.parent))
    shutil.copy2(temporary_zip, output)
    checksum = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(".zip.sha256").write_text(f"{checksum}  {output.name}\n", encoding="ascii")
    (root / "docs/evidence/windows-pe-audit.json").write_text(json.dumps(audit, indent=2) + "\n", encoding="utf-8")
print(output)
