#!/usr/bin/env python3
"""End-to-end CLI test; only generated files in an isolated temporary directory."""
import json
import subprocess
import tempfile
from pathlib import Path

project = Path(__file__).resolve().parents[1]
binary = project / "target" / "debug" / "filespring"
if not binary.exists():
    raise SystemExit("Build first: cargo build")
results = []
with tempfile.TemporaryDirectory(prefix="filespring-cli-") as temporary:
    root = Path(temporary).resolve()
    (root / "in").mkdir()
    (root / "out").mkdir()
    source = root / "in" / "中文测试 & sample.txt"
    source.write_bytes(b"FileSpring CLI verified content\n")

    def run(*args, success=True):
        command = [str(binary), "--root", str(root), "--state", str(root / "state"), *map(str, args)]
        result = subprocess.run(command, capture_output=True, text=True)
        assert (result.returncode == 0) == success, result.stderr
        results.append({"command": list(map(str, args)), "exit_code": result.returncode, "stderr": result.stderr})
        return json.loads(result.stdout) if success and result.stdout.strip().startswith(("{", "[")) else result.stdout

    for action in ("copy", "move"):
        plan = run("plan", action, source, "--to", root / "out")
        assert source.exists() and not (root / "out" / source.name).exists()
        run("apply", plan["id"], success=False)
        completed = run("apply", plan["id"], "--confirm")
        assert completed["state"] == "Completed"
        assert (root / "out" / source.name).read_bytes() == b"FileSpring CLI verified content\n"
        assert source.exists() == (action == "copy")
        assert run("show", plan["id"])["state"] == "Completed"
        undone = run("undo", plan["id"], "--confirm")
        assert undone["state"] == "RolledBack"
        assert source.read_bytes() == b"FileSpring CLI verified content\n"
        assert not (root / "out" / source.name).exists()
    assert len(run("history")) == 2

output = project / "docs" / "evidence" / "cli-smoke.json"
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps({"result": "passed", "fixture": "generated temporary files only", "checks": results}, ensure_ascii=False, indent=2) + "\n")
print(f"CLI copy/move/confirmation/history/undo passed ({len(results)} subprocess commands)")
