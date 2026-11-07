#!/usr/bin/env python3
"""Generate fixtures inside this project's demo/ only. Existing data is preserved."""
from pathlib import Path
import argparse
import csv
import io
import struct
import zlib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "demo")
root = parser.parse_args().output.resolve()
inbox = root / "Inbox"
library = root / "Library"
inbox.mkdir(parents=True, exist_ok=True)
library.mkdir(parents=True, exist_ok=True)

def create(path, data):
    try:
        with path.open("xb") as stream:
            stream.write(data)
    except FileExistsError:
        pass

create(inbox / "欢迎使用 FileSpring.md", "# FileSpring\n\n这些文件由项目自动生成，可用于复制、移动和撤销测试。\n\n1. 在右栏打开 Library。\n2. 在左栏选择文件，点击复制。\n3. 查看预演后确认执行。\n4. 在操作历史中撤销。\n".encode())
for i in range(1, 15):
    create(inbox / f"项目笔记 {i:02}.txt", f"FileSpring isolated fixture {i}\nA native workspace for your files.\n".encode())
output = io.StringIO()
writer = csv.writer(output)
writer.writerows([["name", "bytes"], ["example", 1024], ["sample", 2048]])
create(inbox / "文件清单.csv", output.getvalue().encode())
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
pixels = b"".join(b"\0" + bytes([55, 170, 145]) * 128 for _ in range(128))
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 128, 128, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")
create(inbox / "绿色样图.png", png)
create(library / "这里接收文件.txt", b"Destination folder for isolated transfer tests.\n")
print(root)
