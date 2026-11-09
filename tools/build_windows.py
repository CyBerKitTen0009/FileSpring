#!/usr/bin/env python3
"""Native Windows x64 checks, release build, and portable ZIP packaging."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--offline", action="store_true")
args = parser.parse_args()
if sys.platform != "win32":
    parser.error("Run on Windows with Rust MSVC, C++ Build Tools and Python 3.11+. For a pre-provisioned cross SDK use cross_windows.py.")
root = Path(__file__).resolve().parents[1]
os.environ.setdefault("CARGO_HOME", str(root / ".cache/cargo"))
target = "x86_64-pc-windows-msvc"


def run(command, log=None):
    print("Running: " + subprocess.list2cmdline(command), flush=True)
    if log:
        with log.open("w", encoding="utf-8") as stream:
            result = subprocess.run(command, cwd=root, stdout=stream, stderr=subprocess.STDOUT)
        if result.returncode:
            print(log.read_text(encoding="utf-8", errors="replace")[-12000:])
            raise SystemExit(result.returncode)
    else:
        subprocess.run(command, cwd=root, check=True)


if not args.offline:
    # License collection uses all-platform metadata. A target-limited fetch
    # leaves those crate sources absent on a clean Windows builder.
    run(["cargo", "fetch", "--locked"])
run([sys.executable, "tools/check.py", "--target", target])
run(["cargo", "build", "--offline", "--locked", "--release", "--target", target, "--features", "desktop"], root / "docs/evidence/windows-native-build.log")
artifacts = root / "artifacts"
artifacts.mkdir(exist_ok=True)
run([sys.executable, "tools/cli_smoke.py", "--binary", f"target/{target}/debug/filespring.exe", "--output", str(artifacts / "windows-cli-smoke.json")])
run([str(root / f"target/{target}/release/filespring.exe"), "self-test"], artifacts / "windows-native-self-test.json")
run([sys.executable, "tools/package_windows.py"])
