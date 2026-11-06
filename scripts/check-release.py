#!/usr/bin/env python3
"""Check package/embedded versions, and the release tag when provided."""

import os
import plistlib
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], cwd=root, text=True))
version = next(p["version"] for p in metadata["packages"] if Path(p["manifest_path"]).resolve() == root / "Cargo.toml")
with (root / "resources/Info.plist").open("rb") as file:
    embedded = plistlib.load(file)["CFBundleVersion"]
if version != embedded:
    raise SystemExit("Info.plist version must match Cargo.toml")
tag = os.environ.get("RELEASE_TAG")
if tag and tag != "v" + version:
    raise SystemExit(f"Release tag {tag!r} must match package version v{version}")
print(f"Building nelko {version}" + (f" for release {tag}" if tag else " (build-only run)"))
