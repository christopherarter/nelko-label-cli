---
name: open-nelko
description: Install open-nelko and print, preview, or troubleshoot 4x6 PDF labels on compatible Nelko printers over USB or Bluetooth from macOS or Linux. Use for requests such as "print this label" when a compatible label printer is intended.
license: MIT
---

# Print labels with open-nelko

Use the local `open-nelko` executable to print PDF labels. This is an independent hobby project, unaffiliated with Nelko, supplied AS IS without warranty or guaranteed support. The default profile is `nelko-pl70e-bt`, for 4×6 labels at 203 dpi. Other models are not automatically compatible. macOS Bluetooth printing has been hardware tested; USB and Linux still need physical qualification.

## Install this skill

This directory is a portable skill: keep `SKILL.md`, `LICENSE`, and `scripts/install.py` together. If reading it from a URL, download the whole [skill directory](https://github.com/christopherarter/open-nelko-cli/tree/main/skills/open-nelko), or clone the repository and copy `skills/open-nelko`. Put it in the agent's supported skill directory using that agent's normal installation mechanism. No particular agent, tool names, configuration format, or global skill path is required. Without skill discovery support, read this file as instructions and retain its supporting script locally.

## Set up the executable

Run commands on the computer with access to the printer. A remote agent or container without USB/Bluetooth access can prepare the label, but must hand the print command to that computer.

1. Look for `open-nelko` on `PATH`, then at `~/.local/bin/open-nelko`. Run `--version` and `--help`. Reuse a working installation; do not upgrade it on every print request.
2. If it is missing, run the bundled installer, resolving `SKILL_DIR` to this skill's actual directory:

   ```sh
   python3 "$SKILL_DIR/scripts/install.py"
   ```

   The optional installer needs Python 3.9+ and internet access. It downloads the latest stable GitHub release for this computer, verifies SHA-256 before executing anything, installs to `~/.local/bin/open-nelko`, and keeps the full package and license files under `~/.local/share/open-nelko/releases` (or `$XDG_DATA_HOME/open-nelko/releases`). It needs neither administrator access nor Rust and does not edit shell startup files. Use the absolute executable path it prints when the bin directory is absent from `PATH`.

   Options include `--version v0.1.0`, `--bin-dir DIR`, `--data-dir DIR`, and offline installation with `--archive /path/to/RELEASE.tar.gz` plus its adjacent `.sha256` file. Run `--help` for details. An explicit version can select a prerelease; the default excludes prereleases. No release, missing assets, unsupported platforms, or checksum failures stop installation. Never substitute an unverified download or silently build from source. If no release is published yet, explain that the maintainer must publish one and let its asset builds finish.

3. Run `open-nelko profiles` and `open-nelko print --help` using the resolved executable path. The CLI itself does not require Python. If Python is unavailable, use the [manual release installation instructions](https://github.com/christopherarter/open-nelko-cli#install), including checksum verification.

## Handle “print this label”

A user's request to print authorizes one copy of the specified label on the intended printer. Preserve any already supplied page, copy count, printer, and connection preference. Ask only for missing choices that affect the result, such as several candidate files or printers. Do not require an extra confirmation when those choices are clear.

1. Resolve the requested local PDF or attachment to an absolute path. Keep shipping labels, addresses, and generated previews local and out of source repositories or uploads. Use a temporary directory for previews and exported jobs.
2. Use the known connection and printer when provided. Otherwise enumerate USB first; if no compatible printer is available there, scan Bluetooth. Discovery does not print:

   ```sh
   open-nelko devices --transport usb
   open-nelko devices --transport bluetooth --seconds 15
   ```

   Pick an unambiguous compatible device from the output. Use the USB serial or `vendor:product` identifier, or the Bluetooth peripheral ID/advertised name, as `--device`. Ask when several devices remain plausible. A Bluetooth name alone does not establish that an unsupported model speaks the right protocol. Do not send jobs to unrelated printers.
3. Validate the selected page and settings with a dry run. Replace the sample path, transport, and selector with actual values:

   ```sh
   open-nelko print "/absolute/path/label.pdf" --transport bluetooth --device "PRINTER_ID" --copies 1 --dry-run
   ```

   This does not contact hardware. Multi-page PDFs require a one-based `--page N`; ask which page if the user has not specified it. The CLI accepts portrait or landscape 4×6 pages and rejects Letter/A4. Do not shrink, crop, or print all pages to work around a size/page error without the user's instruction.
4. For a requested preview or an uncertain layout, generate and inspect a PNG before printing:

   ```sh
   open-nelko preview "/absolute/path/label.pdf" --output "/temporary/directory/label.png"
   ```

   Pass the same `--page N`, `--threshold N`, and global `--profile NAME` to preview/dry-run/print when used. Default settings usually suffice; arbitrary threshold changes can make barcodes unreadable.
5. Execute the validated print command **once**, removing only `--dry-run`:

   ```sh
   open-nelko print "/absolute/path/label.pdf" --transport bluetooth --device "PRINTER_ID" --copies 1
   ```

   Use `--transport usb` for USB. The default is one copy; request extra copies only when the user asks for them. Report that the job was sent, with the connection and copy count. A successful transfer does not confirm a physical label printed; claim physical success only when the user or a hardware observation confirms it.

## Troubleshoot without duplicate labels

- If discovery fails, check power, labels, cable/adapter, and whether another app is connected. Bluetooth discovery can be extended with `--seconds 15`; `devices --transport bluetooth --all` shows other nearby devices without printing.
- macOS requires Bluetooth permission for the terminal or agent app launching the executable. Linux Bluetooth requires the system BlueZ and D-Bus services; USB needs the distribution's device permissions. Diagnose the actual error before changing system packages or permissions. Do not run the printer CLI as root as a routine workaround.
- `open-nelko status --device "PRINTER_ID"` queries Bluetooth TSPL status without printing; `inspect` lists its services. Do not guess a writable characteristic or override `--characteristic` without identifying the printer's TSPL data channel.
- After any uncertain or partial print attempt, stop and report the result. Do not automatically retry, switch connections, or issue a test label: that can duplicate the shipping label. For a partial transfer, have the user power-cycle the printer to clear its receive buffer before an explicitly requested retry.
- Linux cannot open password-protected PDFs. Other command languages, Bluetooth Classic-only printers, and unlisted profiles need adapter development rather than improvised raw commands.

Project documentation and releases: [open-nelko-cli](https://github.com/christopherarter/open-nelko-cli). Project terms: [MIT license](LICENSE) and [affiliation/hobby-project notice](https://github.com/christopherarter/open-nelko-cli/blob/main/NOTICE.md).
