# nelko-cli

Print 4×6 PDF shipping labels to a Nelko PL70e-BT from your Mac's terminal, over USB or Bluetooth.

```sh
nelko devices
nelko preview label.pdf --output label.png
nelko print label.pdf --transport usb
nelko print label.pdf --transport bluetooth
```

This is an independent, early-stage CLI for the PL70e-BT. macOS is the initial supported platform. Hardware qualification is tracked in [HARDWARE.md](HARDWARE.md). Other Nelko models can use different protocols.

## Install

Release archives contain a single `nelko` executable with its Rust dependencies compiled in. PDF rendering, Bluetooth and USB use frameworks already included in macOS. Running a release does not require Rust, Homebrew, Python, libusb or a vendor driver.

Download the macOS archive from GitHub Releases once a release is published, extract it, then run `./nelko --help`. You can put the executable in any directory on your `PATH`.

The release workflow builds a universal executable for Apple silicon and Intel Macs. Releases are ad-hoc signed. Developer ID signing and notarization are not configured. The minimum deployment target is macOS 12.

To build from source, install Rust 1.89 or later and Apple's command line developer tools, then run:

```sh
cargo build --release --locked --target-dir target
./target/release/nelko --help
```

## USB

Turn on the printer, load 4×6 labels and connect its USB cable. A USB-C adapter may be needed.

```sh
nelko devices
nelko print label.pdf --transport usb
```

USB printing sends TSPL directly to the printer's bulk endpoint. It does not create a macOS print queue or use an installed printer driver. If another app has claimed the printer, close it and let existing print jobs finish.

When more than one printer is connected, select its serial number or `vendor:product` identifier from `devices`:

```sh
nelko print label.pdf --transport usb --device SERIAL_NUMBER
```

## Bluetooth

```sh
nelko devices --transport bluetooth
nelko inspect --device PL70e-BT
nelko status
nelko print label.pdf --transport bluetooth --device PL70e-BT
```

Allow Bluetooth access when macOS asks. If access was denied, enable it for your terminal or the app launching the CLI under System Settings → Privacy & Security → Bluetooth. Connect through the CLI; pairing in macOS settings is not required.

The default device name is `PL70e-BT`. Bluetooth names use an unambiguous prefix match, ignoring punctuation and case, so names such as `PL70e-BT_4F03-LE` work automatically. Use the peripheral ID printed by `devices` to select between printers with identical names. `--seconds 15` extends discovery; `devices --transport bluetooth --all` includes other nearby devices.

Bluetooth data is split into chunks capped by the negotiated MTU. Writes with responses are preferred. Writes without responses use CoreBluetooth backpressure and a configurable delay to avoid overflowing a printer's receive buffer. Defaults are a maximum of 180 bytes per chunk and a 15 ms delay.

The Nelko profile selects the FFF0/FFF2 channel verified on the tested printer. `status` sends a TSPL status query without printing. If automatic channel detection fails, `inspect` lists services and characteristics without sending print data. After identifying the printer's TSPL data characteristic, select it explicitly:

```sh
nelko print label.pdf --transport bluetooth --characteristic FULL_UUID
```

This transport supports transparent TSPL over Bluetooth LE. A Bluetooth Classic-only variant or a printer requiring a proprietary handshake needs additional protocol work. Recognition of a writable characteristic alone does not prove compatibility.

## Check a label before printing

```sh
nelko preview label.pdf --output preview.png
nelko export label.pdf --output label.tspl
nelko print label.pdf --transport usb --dry-run
```

These commands do not contact a printer. The PNG shows the same thresholded pixels used by both transports. Labels render at 203 dpi, 812×1218 dots. Portrait and landscape 4×6 pages fit while preserving their proportions and PDF rotation. Letter and A4 pages are rejected so a full sheet is not silently shrunk onto a label.

Multi-page PDFs require `--page N`. A job prints one selected page; `--copies N` repeats it, with a default of one copy. `--threshold 160` controls the black/white conversion.

Successful transmission means the bytes were sent; it does not confirm that a physical label printed. Partial transfers are reported and never retried automatically, to avoid duplicate shipping labels. Power-cycle the printer before retrying a partial transfer so an incomplete bitmap does not remain in its receive buffer. There is no automatic fallback between transports.

## Add another printer

`nelko profiles` lists built-in profiles. The default is `--profile nelko-pl70e-bt`.

The code separates printer profiles, raster rendering, command encoders and transports. Another transparent TSPL printer generally needs a profile and a hardware test. Another command language needs an encoder implementing `LabelEncoder`; USB and Bluetooth can reuse the resulting byte stream. See [Adding printers](docs/ADDING_PRINTERS.md) for the specific extension points and current limits.

## Develop and release

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

CI runs these checks on macOS. Tagging a version such as `v0.1.0` runs the release workflow, which builds both Mac architectures, checks that only Apple system libraries are linked, packages dependency license notices and SHA-256 checksums, and creates a draft GitHub release. A maintainer reviews the draft before publishing.

The release archive contains the binary, this README, the MIT license, hardware qualification notes and third-party license files. Build tools are only needed by contributors and release CI.

## Protocol and implementation

The printer's TSPL bitmap path is shared by both connections. The macOS renderer uses Core Graphics through generated `objc2` bindings; USB uses `nusb`, and Bluetooth uses `btleplug`. `Cargo.lock` fixes the dependency graph used by CI and releases. The binary embeds the Bluetooth usage description required by macOS.

This project is unaffiliated with Nelko. See [CONTRIBUTING.md](CONTRIBUTING.md) to report hardware results without sharing personal shipping labels.
