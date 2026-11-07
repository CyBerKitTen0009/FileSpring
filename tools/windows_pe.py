"""Inspect the generated PE files before putting them in a Windows package."""
import struct
import xml.etree.ElementTree as ET


def inspect_pe(path, subsystem):
    data = path.read_bytes()

    def require(condition, message):
        if not condition:
            raise ValueError(f"{path.name}: {message}")

    def unpack(fmt, offset):
        require(0 <= offset <= len(data) - struct.calcsize(fmt), "truncated PE")
        return struct.unpack_from(fmt, data, offset)

    require(data[:2] == b"MZ", "not a Windows executable")
    pe, = unpack("<I", 0x3C)
    require(data[pe:pe + 4] == b"PE\0\0", "missing PE signature")
    machine, sections = unpack("<HH", pe + 4)
    optional_size, = unpack("<H", pe + 20)
    optional = pe + 24
    magic, = unpack("<H", optional)
    actual_subsystem, = unpack("<H", optional + 68)
    require(machine == 0x8664 and magic == 0x20B, "expected Windows x64 PE32+")
    require(actual_subsystem == subsystem, f"wrong subsystem {actual_subsystem}")
    section_table = optional + optional_size

    def offset(rva):
        for index in range(sections):
            virtual_size, address, size, pointer = unpack("<IIII", section_table + index * 40 + 8)
            if address <= rva < address + max(virtual_size, size):
                require(rva - address < size, "RVA is not backed by file data")
                return pointer + rva - address
        raise ValueError(f"{path.name}: unmapped RVA {rva}")

    def directory(index):
        return unpack("<II", optional + 112 + index * 8)

    imports = []
    imports_rva, imports_size = directory(1)
    if imports_rva:
        base = offset(imports_rva)
        for index in range(imports_size // 20):
            descriptor = unpack("<IIIII", base + index * 20)
            if not any(descriptor):
                break
            start = offset(descriptor[3])
            end = data.find(b"\0", start, start + 512)
            require(end >= start, "invalid import name")
            imports.append(data[start:end].decode("ascii"))
    forbidden = ("vcruntime", "msvcp", "concrt", "libgcc", "libstdc++", "libwinpthread", "sqlite3")
    require(not any(name.lower().startswith(forbidden) for name in imports), "non-system runtime DLL dependency")
    delay_rva, delay_size = directory(13)
    require(not delay_rva and not delay_size, "delay imports need a separate runtime audit")
    resource_rva, _ = directory(2)
    require(resource_rva != 0, "embedded manifest missing")
    resource = offset(resource_rva)

    def resource_entry(relative, wanted=None):
        named, numbered = unpack("<HH", resource + relative + 12)
        for index in range(named + numbered):
            name, value = unpack("<II", resource + relative + 16 + index * 8)
            if not name & 0x80000000 and (wanted is None or name == wanted):
                return value
        raise ValueError(f"{path.name}: manifest resource missing")

    type_entry = resource_entry(0, 24)  # RT_MANIFEST
    require(type_entry & 0x80000000, "invalid manifest type")
    name_entry = resource_entry(type_entry & 0x7FFFFFFF, 1)
    require(name_entry & 0x80000000, "invalid manifest name")
    language_entry = resource_entry(name_entry & 0x7FFFFFFF)
    require(not language_entry & 0x80000000, "invalid manifest language")
    manifest_rva, manifest_size = unpack("<II", resource + language_entry)
    manifest_start = offset(manifest_rva)
    manifest = ET.fromstring(data[manifest_start:manifest_start + manifest_size])
    supported = [node.attrib.get("Id") for node in manifest.iter() if node.tag.endswith("}supportedOS")]
    require("{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" in supported, "Windows 10/11 compatibility manifest missing")
    require(any(node.tag.endswith("}longPathAware") and node.text == "true" for node in manifest.iter()), "longPathAware missing")
    require(any(node.tag.endswith("}requestedExecutionLevel") and node.attrib.get("level") == "asInvoker" for node in manifest.iter()), "expected non-elevated launch")
    return {"machine": "x86_64", "subsystem": actual_subsystem, "imports": sorted(set(imports)), "windows_10_11_manifest": True, "long_path_aware": True, "execution_level": "asInvoker", "runtime_tested": False}
