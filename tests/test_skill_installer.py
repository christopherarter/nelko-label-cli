"""Exercise installation without network access or printer side effects."""

import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError


SCRIPT = Path(__file__).resolve().parents[1] / "skills/open-nelko/scripts/install.py"
SPEC = importlib.util.spec_from_file_location("skill_installer", SCRIPT)
installer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(installer)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.target = installer.host_platform()
        self.name = f"open-nelko-cli-v0.1.0-{self.target}"
        self.archive = self.root / f"{self.name}.tar.gz"
        self.checksum = self.root / f"{self.archive.name}.sha256"
        self.bin_dir = self.root / "path with spaces/bin"
        self.data_dir = self.root / "path with spaces/share"

    def package(self, extra=(), version="0.1.0"):
        files = {
            "open-nelko": f"#!/bin/sh\nprintf 'open-nelko {version}\\n'\n".encode(),
            "LICENSE": b"MIT license fixture\n",
            "NOTICE.md": b"Independent hobby project\n",
            "THIRD_PARTY_NOTICES.txt": b"Dependency notices\n",
            "licenses/dependency/LICENSE": b"Third-party license fixture\n",
        }
        with tarfile.open(self.archive, "w:gz") as archive:
            for name, data in files.items():
                entry = tarfile.TarInfo(f"{self.name}/{name}")
                entry.size = len(data)
                archive.addfile(entry, io.BytesIO(data))
            for entry, data in extra:
                archive.addfile(entry, io.BytesIO(data) if data is not None else None)
        digest = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        self.checksum.write_text(f"{digest}  {self.archive.name}\n")
        return digest

    def install(self):
        with contextlib.redirect_stdout(io.StringIO()):
            return installer.install(self.archive, self.checksum, self.target, self.bin_dir, self.data_dir)

    def assert_no_install(self):
        self.assertFalse(self.bin_dir.exists())
        self.assertFalse(self.data_dir.exists())

    def test_detect_supported_platforms(self):
        for system, machine, expected in [
            ("Darwin", "arm64", "macos-universal"),
            ("Darwin", "x86_64", "macos-universal"),
            ("Linux", "x86_64", "linux-x86_64"),
            ("Linux", "aarch64", "linux-arm64"),
        ]:
            with self.subTest(system=system, machine=machine), patch.object(installer.platform, "system", return_value=system), patch.object(installer.platform, "machine", return_value=machine), patch.object(installer.platform, "mac_ver", return_value=("12.0", (), "")):
                self.assertEqual(installer.host_platform(), expected)

    def test_unsupported_system_or_architecture_is_rejected(self):
        for system, machine in [("Windows", "AMD64"), ("Linux", "armv7l")]:
            with self.subTest(system=system), patch.object(installer.platform, "system", return_value=system), patch.object(installer.platform, "machine", return_value=machine):
                with self.assertRaises(installer.InstallError):
                    installer.host_platform()

    def test_old_macos_is_rejected(self):
        with patch.object(installer.platform, "system", return_value="Darwin"), patch.object(installer.platform, "machine", return_value="x86_64"), patch.object(installer.platform, "mac_ver", return_value=("11.7", (), "")):
            with self.assertRaisesRegex(installer.InstallError, "12 or later"):
                installer.host_platform()

    def test_install_and_repeat_preserve_all_licenses(self):
        self.package()
        executable = self.install()
        self.assertEqual(subprocess.check_output([str(executable), "--version"], text=True).strip(), "open-nelko 0.1.0")
        self.assertTrue(executable.stat().st_mode & 0o111)
        packages = list((self.data_dir / "releases").iterdir())
        self.assertEqual(len(packages), 1)
        self.assertEqual((packages[0] / "licenses/dependency/LICENSE").read_bytes(), b"Third-party license fixture\n")
        self.assertTrue((packages[0] / "NOTICE.md").is_file())
        self.install()
        self.assertEqual(list((self.data_dir / "releases").iterdir()), packages)
        self.assertEqual(list(self.bin_dir.iterdir()), [executable])

    def test_bad_checksum_never_executes_or_overwrites_existing_binary(self):
        self.package()
        self.bin_dir.mkdir(parents=True)
        existing = self.bin_dir / "open-nelko"
        existing.write_bytes(b"previous binary")
        self.checksum.write_text(f"{'0' * 64}  {self.archive.name}\n")
        with patch.object(installer.subprocess, "run") as run:
            with self.assertRaisesRegex(installer.InstallError, "SHA-256 mismatch"):
                self.install()
            run.assert_not_called()
        self.assertEqual(existing.read_bytes(), b"previous binary")
        self.assertFalse(self.data_dir.exists())

    def test_checksum_for_different_filename_is_rejected(self):
        digest = self.package()
        self.checksum.write_text(f"{digest}  other.tar.gz\n")
        with self.assertRaisesRegex(installer.InstallError, "exact filename"):
            self.install()
        self.assert_no_install()

    def test_archive_traversal_links_and_duplicate_paths_are_rejected(self):
        for name, kind in [
            (f"{self.name}/../../outside", tarfile.REGTYPE),
            ("/absolute/path", tarfile.REGTYPE),
            (f"{self.name}/link", tarfile.SYMTYPE),
            (f"{self.name}/hardlink", tarfile.LNKTYPE),
            (f"{self.name}/open-nelko", tarfile.REGTYPE),
        ]:
            with self.subTest(name=name):
                entry = tarfile.TarInfo(name)
                entry.type = kind
                entry.linkname = "../../outside"
                self.package([(entry, b"" if kind == tarfile.REGTYPE else None)])
                with patch.object(installer.subprocess, "run") as run:
                    with self.assertRaisesRegex(installer.InstallError, "Unsafe or unexpected"):
                        self.install()
                    run.assert_not_called()
                self.assert_no_install()

    def test_wrong_binary_version_is_rejected_before_installation(self):
        self.package(version="9.9.9")
        with self.assertRaisesRegex(installer.InstallError, "version does not match"):
            self.install()
        self.assert_no_install()

    def test_wrong_platform_or_requested_version_is_rejected(self):
        self.package()
        with self.assertRaisesRegex(installer.InstallError, "filename"):
            installer.install(self.archive, self.checksum, "wrong-platform", self.bin_dir, self.data_dir)
        with self.assertRaisesRegex(installer.InstallError, "version does not match"):
            installer.install(self.archive, self.checksum, self.target, self.bin_dir, self.data_dir, "v9.9.9")
        self.assert_no_install()

    def test_public_latest_download_then_install(self):
        self.package()
        release = {"tag_name": "v0.1.0", "draft": False, "prerelease": False, "assets": [{"name": self.archive.name}, {"name": self.checksum.name}]}
        requested = []

        def response(request, timeout):
            requested.append(request.full_url)
            if request.full_url.endswith("/releases/latest"):
                return io.BytesIO(json.dumps(release).encode())
            if request.full_url.endswith(".sha256"):
                return io.BytesIO(self.checksum.read_bytes())
            return io.BytesIO(self.archive.read_bytes())

        downloads = self.root / "downloads"
        downloads.mkdir()
        with patch.object(installer, "urlopen", side_effect=response), contextlib.redirect_stdout(io.StringIO()):
            archive, checksum = installer.download_release(downloads, self.target, None)
            binary = installer.install(archive, checksum, self.target, self.bin_dir, self.data_dir)
        self.assertTrue(binary.is_file())
        self.assertEqual(len(requested), 3)
        self.assertTrue(requested[0].endswith("/releases/latest"))
        self.assertTrue(all(url.startswith("https://") for url in requested))

    def test_no_release_incomplete_assets_and_implicit_prerelease_fail(self):
        with patch.object(installer, "urlopen", side_effect=HTTPError("url", 404, "missing", {}, None)):
            with self.assertRaisesRegex(installer.InstallError, "Publish a release"):
                installer.download_release(self.root, self.target, None)
        for release in [
            {"tag_name": "v0.1.0", "assets": []},
            {"tag_name": "v0.1.0-beta.1", "prerelease": True},
        ]:
            with self.subTest(release=release), patch.object(installer, "fetch", return_value=json.dumps(release).encode()) as fetch:
                with self.assertRaises(installer.InstallError):
                    installer.download_release(self.root, self.target, None)
                self.assertEqual(fetch.call_count, 1)
        self.assert_no_install()

    def test_cli_offline_install_uses_adjacent_checksum(self):
        self.package()
        subprocess.run([sys.executable, str(SCRIPT), "--archive", str(self.archive), "--bin-dir", str(self.bin_dir), "--data-dir", str(self.data_dir)], check=True, capture_output=True, text=True)
        self.assertTrue((self.bin_dir / "open-nelko").is_file())


if __name__ == "__main__":
    unittest.main()
