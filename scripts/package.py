#!/usr/bin/env python3
"""Package a portable executable, dependency licenses and SHA-256 checksum."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

PLATFORMS = {
    "aarch64-apple-darwin": "macos-arm64",
    "x86_64-apple-darwin": "macos-x86_64",
    "universal-apple-darwin": "macos-universal",
    "x86_64-unknown-linux-musl": "linux-x86_64",
    "aarch64-unknown-linux-musl": "linux-arm64",
}


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=PLATFORMS, required=True)
    parser.add_argument("--output", type=Path, default=Path("dist"))
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    binary = args.binary.resolve()
    if "apple-darwin" in args.target:
        architectures = set(command("lipo", "-archs", str(binary)).split())
        expected = {"aarch64-apple-darwin": {"arm64"}, "x86_64-apple-darwin": {"x86_64"}, "universal-apple-darwin": {"arm64", "x86_64"}}[args.target]
        if architectures != expected:
            raise SystemExit(f"Wrong architectures: expected {expected}, got {architectures}")
        for line in command("otool", "-L", str(binary)).splitlines():
            if line.startswith("\t"):
                dependency = line.strip().split()[0]
                if not dependency.startswith(("/System/Library/", "/usr/lib/")):
                    raise SystemExit(f"Non-system runtime dependency: {dependency}")
        if "__info_plist" not in command("otool", "-l", str(binary)):
            raise SystemExit("Executable is missing embedded macOS Bluetooth metadata")
    else:
        header = binary.read_bytes()[:64]
        expected_machine = 62 if args.target.startswith("x86_64") else 183
        if header[:6] != b"\x7fELF\x02\x01" or int.from_bytes(header[18:20], "little") != expected_machine:
            raise SystemExit("Wrong ELF architecture for requested Linux target")
        if "INTERP" in command("readelf", "-l", str(binary)) or "(NEEDED)" in command("readelf", "-d", str(binary)):
            raise SystemExit("Linux release must be statically linked")

    platform = "aarch64-apple-darwin" if args.target == "universal-apple-darwin" else args.target
    metadata = json.loads(command("cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", platform))
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

    name = f"open-nelko-cli-v{version}-{PLATFORMS[args.target]}"
    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        stage = Path(temporary) / name
        stage.mkdir()
        shutil.copy2(binary, stage / "open-nelko")
        for document in ["README.md", "LICENSE", "NOTICE.md", "HARDWARE.md", "CONTRIBUTING.md", "VALIDATION.md"]:
            shutil.copy2(root / document, stage / document)
        shutil.copytree(root / "docs", stage / "docs")
        shutil.copytree(root / "skills", stage / "skills", ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
        notices = ["Third-party dependencies compiled into open-nelko\n"]
        for package in sorted((packages[i] for i in included if i != root_id), key=lambda p: (p["name"], p["version"])):
            source = Path(package["manifest_path"]).parent
            licenses = {p for p in source.rglob("*") if p.is_file() and any(word in p.name.upper() for word in ("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "OFL", "NOTICE")) and p.suffix.lower() not in (".rs", ".c", ".h", ".py", ".sh", ".js", ".ts")}
            if package.get("license_file"):
                licenses.add(source / package["license_file"])
            if not licenses:
                vendored = root / "resources" / "licenses" / "crates" / f"{package['name']}-{package['version']}"
                licenses.update(vendored.glob("LICENSE*"))
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
                relative = license_file.relative_to(source) if license_file.is_relative_to(source) else Path(license_file.name)
                destination = folder / relative
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(license_file, destination)
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
