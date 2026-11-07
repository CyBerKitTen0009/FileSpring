#!/usr/bin/env python3
"""Run repeatable local checks and keep their exact outputs in docs/evidence/."""
import os
import argparse
import platform
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target")
parser.add_argument("--online", action="store_true")
args = parser.parse_args()
os.environ.setdefault("CARGO_HOME", str(root / ".cache" / "cargo"))
evidence = root / "docs" / "evidence"
evidence.mkdir(parents=True, exist_ok=True)
steps = [
    ("format.log", ["cargo", "fmt", "--all", "--check"]),
    ("clippy.log", ["cargo", "clippy", "--offline", "--locked", "--all-targets", "--features", "desktop", "--", "-D", "warnings"]),
    ("tests.log", ["cargo", "test", "--offline", "--locked", "--features", "desktop", "--", "--nocapture"]),
    ("build.log", ["cargo", "build", "--offline", "--locked", "--features", "desktop"]),
]
for name, command in steps:
    if args.online:
        command = [part for part in command if part != "--offline"]
    if args.target and name != "format.log":
        position = command.index("--") if "--" in command else len(command)
        command[position:position] = ["--target", args.target]
    path = evidence / name
    with path.open("w", encoding="utf-8") as log:
        log.write(f"Platform: {platform.platform()}\nCommand: {' '.join(command)}\n\n")
        log.flush()
        result = subprocess.run(command, cwd=root, stdout=log, stderr=subprocess.STDOUT)
        log.write(f"\nExit code: {result.returncode}\n")
    print(f"{name}: exit {result.returncode}", flush=True)
    if result.returncode:
        print(path.read_text(encoding="utf-8", errors="replace")[-12000:])
        raise SystemExit(result.returncode)
