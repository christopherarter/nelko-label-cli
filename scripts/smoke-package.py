#!/usr/bin/env python3
"""Verify checksums and run the extracted release on a synthetic label."""

import hashlib
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile


def fixture():
    stream = b"0 g\n0 396 36 36 re f\n"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 288 432] /Resources << >> /Contents 4 0 R >>",
        f"<< /Length {len(stream)} >>\nstream\n".encode() + stream + b"endstream",
    ]
    pdf = bytearray(b"%PDF-1.4\n")
    offsets = []
    for index, value in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf.extend(f"{index} 0 obj\n".encode() + value + b"\nendobj\n")
    xref = len(pdf)
    pdf.extend(b"xref\n0 5\n0000000000 65535 f \n")
    for offset in offsets:
        pdf.extend(f"{offset:010} 00000 n \n".encode())
    pdf.extend(f"trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return pdf


def check(archive):
    expected = archive.with_suffix(archive.suffix + ".sha256").read_text().split()
    if expected != [hashlib.sha256(archive.read_bytes()).hexdigest(), archive.name]:
        raise SystemExit(f"Checksum mismatch: {archive}")
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory).resolve()
        with tarfile.open(archive) as package:
            # Compatible with Python 3.10 on Ubuntu 22.04. Allow regular files
            # and directories only, all contained within this temporary folder.
            for member in package.getmembers():
                if not (member.isfile() or member.isdir()) or not (root / member.name).resolve().is_relative_to(root):
                    raise SystemExit(f"Unsafe archive entry: {member.name}")
            package.extractall(root)
        binaries = [p for p in root.glob("*/open-nelko") if p.is_file()]
        if len(binaries) != 1:
            raise SystemExit("Archive must contain one open-nelko executable")
        executable = binaries[0]
        for document in ("LICENSE", "NOTICE.md"):
            if not (executable.parent / document).is_file():
                raise SystemExit(f"Release archive is missing {document}")

        def run(*args):
            return subprocess.check_output([str(executable), *map(str, args)], text=True)

        print(run("--version").strip())
        if "nelko-pl70e-bt" not in run("profiles"):
            raise SystemExit("Missing default printer profile")
        pdf = root / "test.pdf"
        pdf.write_bytes(fixture())
        png = root / "preview.png"
        run("preview", pdf, "--output", png)
        if png.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
            raise SystemExit("Preview is not a PNG")
        job_file = root / "job.tspl"
        run("export", pdf, "--output", job_file)
        job = job_file.read_bytes()
        header = re.search(rb"BITMAP 0,0,(\d+),(\d+),0,", job)
        if not header or (int(header[1]), int(header[2])) != (102, 1218):
            raise SystemExit("Wrong release raster dimensions")
        bitmap = job[header.end():header.end() + 102 * 1218]
        if len(bitmap) != 102 * 1218:
            raise SystemExit("Truncated bitmap")

        def white(x, y):
            return bool(bitmap[y * 102 + x // 8] & (0x80 >> (x % 8)))

        if white(30, 30) or not white(780, 30) or not white(30, 1188):
            raise SystemExit("PDF is flipped, undersized or has a wrong background")
        if "No device was contacted" not in run("print", pdf, "--transport", "usb", "--dry-run"):
            raise SystemExit("Dry run failed")
        skill = executable.parent / "skills/open-nelko"
        for name in ("SKILL.md", "LICENSE", "scripts/install.py"):
            if not (skill / name).is_file():
                raise SystemExit(f"Release is missing skill file: {name}")
        bin_dir = root / "installed/bin"
        subprocess.run([
            sys.executable, str(skill / "scripts/install.py"),
            "--archive", str(archive.resolve()),
            "--bin-dir", str(bin_dir), "--data-dir", str(root / "installed/share"),
        ], check=True)
        installed = bin_dir / "open-nelko"
        if subprocess.check_output([str(installed), "--version"], text=True) != run("--version"):
            raise SystemExit("Installed executable version differs from packaged executable")
        if "No device was contacted" not in subprocess.check_output([str(installed), "print", str(pdf), "--transport", "usb", "--dry-run"], text=True):
            raise SystemExit("Installed executable dry run failed")
        print(f"Verified {archive.name}: checksum, executable, PDF preview, TSPL raster, dry run and skill installation")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit("usage: smoke-package.py ARCHIVE [ARCHIVE...]")
    for path in sys.argv[1:]:
        check(Path(path))
