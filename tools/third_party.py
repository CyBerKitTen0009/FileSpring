#!/usr/bin/env python3
"""Collect declared SPDX licenses for the exact all-feature dependency graph."""
import json
import os
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
os.environ.setdefault("CARGO_HOME", str(root / ".cache" / "cargo"))
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--offline", "--locked", "--all-features", "--format-version", "1"], cwd=root))
packages = sorted((p for p in metadata["packages"] if p["source"]), key=lambda p: (p["name"], p["version"]))
lines = ["# Third-party notices", "", "Generated from the locked Cargo metadata by `python3 tools/third_party.py`.", "", "This inventory includes platform-specific dependencies, build tools, and test dependencies. It records upstream license declarations; it is not a completed distribution compliance audit. Before public releases, collect all applicable copyright, font, vendored C library, and notice files and review the actual selected license alternatives. No media codecs or OCR models are included in this prototype.", "", "FileSpring itself is GPL-3.0-only. The egui/eframe stack declares MIT OR Apache-2.0. SQLite's bundled sources and each dependency's upstream notices must accompany relevant distributions.", "", "| Package | Version | Declared license |", "| --- | --- | --- |"]
for p in packages:
    license_name = p["license"] or (f"See {p['license_file']}" if p["license_file"] else "UNDECLARED — review required")
    repository = p.get("repository") or f"https://crates.io/crates/{p['name']}/{p['version']}"
    lines.append(f"| [{p['name']}]({repository}) | {p['version']} | {license_name.replace('|', '/')} |")
(root / "THIRD_PARTY_NOTICES.md").write_text("\n".join(lines) + "\n")
print(f"Recorded {len(packages)} dependency license declarations")
