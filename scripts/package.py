#!/usr/bin/env python3
"""Package a macOS executable, system-linkage check, and dependency licenses."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=["aarch64-apple-darwin", "x86_64-apple-darwin", "universal-apple-darwin"], required=True)
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    binary = args.binary.resolve()
    architectures = set(command("lipo", "-archs", str(binary)).split())
    expected = {"aarch64-apple-darwin": {"arm64"}, "x86_64-apple-darwin": {"x86_64"}, "universal-apple-darwin": {"arm64", "x86_64"}}[args.target]
    if architectures != expected:
        raise SystemExit(f"Wrong architectures: expected {expected}, got {architectures}")
    for line in command("otool", "-L", str(binary)).splitlines():
        if line.startswith("\t"):
            dependency = line.strip().split()[0]
            if not dependency.startswith(("/System/Library/", "/usr/lib/")):
                raise SystemExit(f"Non-system runtime dependency: {dependency}")
    # The Bluetooth permission description must survive stripping and lipo.
    if "__info_plist" not in command("otool", "-l", str(binary)):
        raise SystemExit("Executable is missing embedded macOS Bluetooth metadata")

    metadata = json.loads(command("cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", "aarch64-apple-darwin"))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    root_id = metadata["resolve"]["root"]
    version = packages[root_id]["version"]
    # Release code does not link development-only dependencies.
    included = set()
    pending = [root_id]
    while pending:
        current = pending.pop()
        if current in included:
            continue
        included.add(current)
        for dep in nodes[current]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                pending.append(dep["pkg"])

    name = f"nelko-cli-v{version}-{args.target}"
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        stage = Path(temporary) / name
        stage.mkdir()
        shutil.copy2(binary, stage / "nelko")
        for document in ["README.md", "LICENSE", "HARDWARE.md"]:
            shutil.copy2(root / document, stage / document)
        notices = ["Third-party dependencies compiled into nelko\n"]
        for package in sorted((packages[i] for i in included if i != root_id), key=lambda p: (p["name"], p["version"])):
            source = Path(package["manifest_path"]).parent
            licenses = {p for pattern in ["LICENSE*", "LICENCE*", "COPYING*", "COPYRIGHT*"] for p in source.glob(pattern) if p.is_file()}
            if package.get("license_file"):
                licenses.add(source / package["license_file"])
            if not licenses and package.get("repository") == "https://github.com/madsmtm/objc2":
                vcs = json.loads((source / ".cargo_vcs_info.json").read_text())
                upstream = root / "resources" / "licenses" / "objc2" / vcs["git"]["sha1"]
                licenses.update(upstream.glob("LICENSE*"))
                licenses.add(root / "resources" / "licenses" / "objc2" / "MIT.txt")
            if not licenses:
                raise SystemExit(f"No bundled license files for {package['name']}; add attribution before distributing")
            folder = stage / "licenses" / f"{package['name']}-{package['version']}"
            folder.mkdir(parents=True)
            for license_file in sorted(licenses):
                shutil.copy2(license_file, folder / license_file.name)
            (folder / "AUTHORS.txt").write_text("\n".join(package.get("authors", [])) + "\n")
            notices.append(f"{package['name']} {package['version']}\nLicense: {package.get('license') or 'see license files'}\nSource: {package.get('repository') or package.get('source')}\n")
        (stage / "THIRD_PARTY_NOTICES.txt").write_text("\n".join(notices))
        archive = args.output / f"{name}.tar.gz"
        with tarfile.open(archive, "w:gz") as output:
            output.add(stage, arcname=name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum_file = archive.with_suffix(archive.suffix + ".sha256")
    checksum_file.write_text(f"{checksum}  {archive.name}\n")
    print(f"Packaged {archive} ({archive.stat().st_size:,} bytes), with {len(included) - 1} dependency license notices")
    print(f"SHA-256: {checksum}")


if __name__ == "__main__":
    main()
