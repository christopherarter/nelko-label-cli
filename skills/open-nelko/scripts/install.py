#!/usr/bin/env python3
"""Install a verified open-nelko release using Python 3.9+ standard libraries."""

import argparse
import hashlib
import hmac
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

REPOSITORY = "christopherarter/open-nelko-cli"
RELEASES_URL = f"https://github.com/{REPOSITORY}/releases"
TAG = r"v[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"
ARCHIVE = re.compile(
    rf"open-nelko-cli-({TAG})-(macos-universal|linux-x86_64|linux-arm64)\.tar\.gz"
)


class InstallError(Exception):
    """An actionable installation failure."""


def host_platform():
    system, machine = platform.system(), platform.machine().lower()
    if system == "Darwin" and machine in {"arm64", "aarch64", "x86_64", "amd64"}:
        version = platform.mac_ver()[0]
        if version and int(version.split(".")[0]) < 12:
            raise InstallError("open-nelko requires macOS 12 or later.")
        return "macos-universal"
    if system == "Linux":
        if machine in {"x86_64", "amd64"}:
            return "linux-x86_64"
        if machine in {"aarch64", "arm64"}:
            return "linux-arm64"
    raise InstallError(f"No release binary for {system}/{machine}; supported: macOS 12+, Linux x86_64/ARM64.")


def fetch(url, destination=None):
    request = Request(url, headers={
        "Accept": "application/vnd.github+json" if destination is None else "application/octet-stream",
        "User-Agent": "open-nelko-skill-installer",
    })
    try:
        with urlopen(request, timeout=60) as response:
            if destination is None:
                return response.read(2 * 1024 * 1024)
            with destination.open("wb") as output:
                shutil.copyfileobj(response, output)
    except HTTPError as error:
        if error.code == 404:
            raise InstallError(
                f"Release or asset not found. Publish a release and wait for its asset builds to finish: {RELEASES_URL}"
            ) from error
        if error.code in {403, 429}:
            raise InstallError("GitHub denied the download or its public API rate limit was reached. Try later or install a verified local archive with --archive.") from error
        raise InstallError(f"Download failed (HTTP {error.code}): {url}") from error
    except (URLError, TimeoutError) as error:
        raise InstallError(f"Download failed: {error}. Check network/TLS access; verification must remain enabled.") from error


def download_release(directory, target, requested_version):
    endpoint = "latest" if requested_version is None else f"tags/{quote(requested_version, safe='')}"
    release = json.loads(fetch(f"https://api.github.com/repos/{REPOSITORY}/releases/{endpoint}"))
    tag = release.get("tag_name", "")
    if not re.fullmatch(TAG, tag) or release.get("draft") or (requested_version is None and release.get("prerelease")):
        raise InstallError("GitHub did not return a supported published release.")
    if requested_version is not None and tag != requested_version:
        raise InstallError("GitHub returned a different release than requested.")
    filename = f"open-nelko-cli-{tag}-{target}.tar.gz"
    names = {asset["name"] for asset in release.get("assets", [])}
    if not {filename, filename + ".sha256"}.issubset(names):
        raise InstallError(f"Release {tag} is missing the {target} archive/checksum. Wait for the release builds to finish: {RELEASES_URL}/tag/{quote(tag, safe='')}")
    base = f"{RELEASES_URL}/download/{quote(tag, safe='')}/"
    archive = directory / filename
    print(f"Downloading {tag} for {target}...", flush=True)
    fetch(base + quote(filename), archive)
    checksum = directory / (filename + ".sha256")
    fetch(base + quote(checksum.name), checksum)
    return archive, checksum


def verify_archive(archive, checksum):
    text = checksum.read_text(encoding="utf-8")
    match = re.fullmatch(r"([0-9a-fA-F]{64})[ \t]+\*?" + re.escape(archive.name) + r"\s*", text)
    if not match:
        raise InstallError("Checksum file must contain one SHA-256 entry for this archive's exact filename.")
    digest = hashlib.sha256()
    with archive.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    if not hmac.compare_digest(digest.hexdigest(), match[1].lower()):
        raise InstallError("SHA-256 mismatch. Nothing was installed or executed; download a fresh archive and checksum.")
    return digest.hexdigest()


def unpack(archive, directory):
    """Copy only regular files/directories inside the expected package root."""
    package_name = archive.name.removesuffix(".tar.gz")
    seen = set()
    with tarfile.open(archive, "r:gz") as package:
        for member in package:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or "\\" in member.name
                    or not path.parts or path.parts[0] != package_name
                    or path in seen or not (member.isfile() or member.isdir())):
                raise InstallError(f"Unsafe or unexpected archive entry: {member.name}")
            seen.add(path)
            destination = directory.joinpath(*path.parts)
            if member.isdir():
                destination.mkdir(parents=True, exist_ok=True)
            else:
                destination.parent.mkdir(parents=True, exist_ok=True)
                with package.extractfile(member) as source, destination.open("xb") as output:
                    shutil.copyfileobj(source, output)
                destination.chmod(0o755 if path.name == "open-nelko" and len(path.parts) == 2 else 0o644)
    result = directory / package_name
    for name in ("open-nelko", "LICENSE", "NOTICE.md", "THIRD_PARTY_NOTICES.txt"):
        if not (result / name).is_file():
            raise InstallError(f"Archive is missing {name}.")
    if not (result / "licenses").is_dir():
        raise InstallError("Archive is missing dependency licenses.")
    return result


def install(archive, checksum, target, bin_dir, data_dir, requested_version=None):
    match = ARCHIVE.fullmatch(archive.name)
    if not match or match[2] != target:
        raise InstallError(f"Archive filename must identify an open-nelko release for {target}.")
    tag = match[1]
    if requested_version is not None and tag != requested_version:
        raise InstallError("Local archive version does not match --version.")
    digest = verify_archive(archive, checksum)
    with tempfile.TemporaryDirectory(prefix="open-nelko-verify-") as temporary:
        package = unpack(archive, Path(temporary))
        executable = package / "open-nelko"
        result = subprocess.run([str(executable), "--version"], check=True, capture_output=True, text=True, timeout=15)
        if result.stdout.strip() != f"open-nelko {tag[1:]}":
            raise InstallError("Executable version does not match its release archive.")

        # Keep the complete package, including every third-party license. A
        # digest suffix separates rebuilt archives without deleting older ones.
        releases = data_dir / "releases"
        releases.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".install-", dir=releases) as staging:
            staged = Path(staging) / package.name
            shutil.copytree(package, staged)
            (staged / "INSTALL.json").write_text(json.dumps({"version": tag, "sha256": digest}, indent=2) + "\n", encoding="utf-8")
            installed_package = releases / f"{package.name}-{digest[:12]}"
            if installed_package.exists():
                marker = installed_package / "INSTALL.json"
                if not marker.is_file() or json.loads(marker.read_text())["sha256"] != digest:
                    raise InstallError(f"Existing package directory has conflicting metadata: {installed_package}")
            else:
                staged.rename(installed_package)

        # Verify everything before replacing an installed executable. The final
        # replace is atomic even when bin_dir and data_dir are on different disks.
        bin_dir.mkdir(parents=True, exist_ok=True)
        descriptor, candidate = tempfile.mkstemp(prefix=".open-nelko-", dir=bin_dir)
        os.close(descriptor)
        candidate = Path(candidate)
        try:
            shutil.copyfile(executable, candidate)
            candidate.chmod(0o755)
            os.replace(candidate, bin_dir / "open-nelko")
        finally:
            candidate.unlink(missing_ok=True)

    binary = bin_dir / "open-nelko"
    print(f"Installed {result.stdout.strip()}: {binary}")
    print(f"Full package and licenses: {installed_package}")
    print(f"Check: {shlex.quote(str(binary))} --help")
    path_entries = [Path(p).expanduser().resolve() for p in os.environ.get("PATH", "").split(os.pathsep) if p]
    if bin_dir not in path_entries:
        print(f"Use that absolute path, or add this directory to your shell's PATH: {shlex.quote(str(bin_dir))}")
    return binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", help="release tag, e.g. v0.1.0; default: latest stable release")
    parser.add_argument("--archive", type=Path, help="install a local release archive instead of downloading")
    parser.add_argument("--checksum", type=Path, help="local checksum file; default: ARCHIVE.sha256")
    parser.add_argument("--bin-dir", type=Path, default=Path.home() / ".local/bin")
    data_home = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")
    parser.add_argument("--data-dir", type=Path, default=data_home / "open-nelko")
    args = parser.parse_args()
    if args.version is not None and not re.fullmatch(TAG, args.version):
        parser.error("--version must be a release tag such as v0.1.0")
    if args.checksum and not args.archive:
        parser.error("--checksum requires --archive")
    try:
        target = host_platform()
        with tempfile.TemporaryDirectory(prefix="open-nelko-download-") as temporary:
            if args.archive:
                archive = args.archive.expanduser().absolute()
                checksum = args.checksum.expanduser() if args.checksum else archive.with_name(archive.name + ".sha256")
            else:
                archive, checksum = download_release(Path(temporary), target, args.version)
            install(archive, checksum, target, args.bin_dir.expanduser().absolute(), args.data_dir.expanduser().absolute(), args.version)
    except (InstallError, OSError, ValueError, KeyError, tarfile.TarError, subprocess.SubprocessError) as error:
        print(f"Installation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
