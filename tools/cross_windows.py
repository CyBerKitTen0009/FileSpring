#!/usr/bin/env python3
"""Build Windows x64 using an already installed Rust target and MSVC SDK.

Does not install tools, download an SDK, or accept any license. Supply an SDK
that has already been provisioned, in xwin's crt/ and sdk/ layout.
"""
import argparse
import os
from pathlib import Path
import shlex
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--sdk", type=Path, required=True)
parser.add_argument("--llvm", type=Path, required=True, help="LLVM bin directory")
parser.add_argument("--linker", type=Path, required=True, help="lld-link binary")
parser.add_argument("--rust-bin", type=Path, required=True)
parser.add_argument("--check", action="store_true", help="Clippy all targets instead of release build")
parser.add_argument("--tests", action="store_true", help="Compile tests without running Windows executables")
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
sdk, llvm = args.sdk.resolve(), args.llvm.resolve()
target = "x86_64-pc-windows-msvc"
env = dict(os.environ)
env["CARGO_HOME"] = str(root / ".cache/cargo")
env["PATH"] = str(args.rust_bin.resolve()) + os.pathsep + str(llvm) + os.pathsep + env["PATH"]
env["RUSTC"] = str(args.rust_bin.resolve() / "rustc")
env["CC_x86_64_pc_windows_msvc"] = str(llvm / "clang-cl")
env["AR_x86_64_pc_windows_msvc"] = str(llvm / "llvm-lib")
includes = [sdk / "crt/include", *[sdk / "sdk/include" / name for name in ["ucrt", "shared", "um", "winrt"]]]
libraries = [sdk / "crt/lib/x86_64", sdk / "sdk/lib/um/x86_64", sdk / "sdk/lib/ucrt/x86_64"]
for path in [*includes, *libraries, args.linker, llvm / "clang-cl", args.rust_bin / "cargo"]:
    if not path.exists():
        parser.error(f"Missing preinstalled prerequisite: {path}")
env["CC_SHELL_ESCAPED_FLAGS"] = "1"
env["CFLAGS_x86_64_pc_windows_msvc"] = shlex.join([f"-imsvc{path}" for path in includes])
env["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"] = str(args.linker.resolve())
env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join(["-C", "target-feature=+crt-static", *[f"-Lnative={path}" for path in libraries]])
# Rust's lld build cannot merge manifests without mt.exe. Compile our one
# manifest directly into a native resource instead; the PE audit verifies it.
resource_dir = root / "target/windows-resource"
resource_dir.mkdir(parents=True, exist_ok=True)
resource_script = resource_dir / "filespring.rc"
resource_script.write_text('1 24 "' + (root / "packaging/windows/filespring.manifest").as_posix() + '"\n', encoding="utf-8")
resource = resource_dir / "filespring.res"
subprocess.run([str(llvm / "llvm-rc"), "/fo", resource.name, resource_script.name], cwd=resource_dir, check=True)
env["FILESPRING_WINDOWS_RESOURCE"] = str(resource)
command = [str(args.rust_bin / "cargo"), "clippy" if args.check else "test" if args.tests else "build", "--offline", "--locked", "--target", target, "--features", "desktop"]
if args.check:
    command += ["--all-targets", "--", "-D", "warnings"]
elif args.tests:
    command += ["--no-run"]
else:
    command += ["--release"]
log_path = root / "docs/evidence" / ("windows-cross-clippy.log" if args.check else "windows-test-build.log" if args.tests else "windows-cross-build.log")
with log_path.open("w", encoding="utf-8") as log:
    log.write("Cross-compiled on macOS; this is not a Windows runtime test.\n")
    log.write("Command: " + shlex.join(command) + "\n\n")
    log.flush()
    result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    log.write(f"\nExit code: {result.returncode}\n")
print(f"{log_path.name}: exit {result.returncode}", flush=True)
if result.returncode:
    print(log_path.read_text(encoding="utf-8", errors="replace")[-18000:])
raise SystemExit(result.returncode)
