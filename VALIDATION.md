# Initial validation, 2026-11-06

Local host: Apple silicon Mac, macOS 26.5.2, Rust 1.96.1.

- Formatting check passed.
- Clippy passed for all targets and features with warnings treated as errors.
- All 10 tests passed, including upright PDF rendering, explicit page selection, 300 dpi rendering, bitmap packing, channel selection and CLI input validation.
- Both Apple silicon and Intel Mac release targets compiled. The universal executable passed ad-hoc signature verification and the package check for Apple system library dependencies only.
- The release archive's SHA-256 checksum was checked. Its extracted executable ran successfully, and its rendered preview exactly matched the visually verified raster.
- Bluetooth discovery, service inspection and the FFF0/FFF2 TSPL status query were verified on a PL70e-BT. One copy of the requested shipping label was sent; the user confirmed it printed correctly at full 4×6 size.

USB transmission is implemented but has not been tested with connected hardware. The Intel build was cross-compiled, not executed on an Intel Mac. CI and the release workflow are prepared locally; they have not run on GitHub. The declared minimum Rust version has a CI check but was not separately installed for the local run.

## Cross-platform release update

The Linux renderer is compiled into the executable. Its synthetic PDF orientation, page selection and 300 dpi tests pass both through the portable macOS test feature and in a native ARM64 musl Linux container (10 total tests per run). The default macOS renderer also continues to pass all 10 tests. The portable renderer's output for a real shipping-label PDF was visually checked without printing another label.

Release jobs validate static Linux linkage, macOS system-library linkage, checksums, and the extracted executable's PDF preview/export/dry-run path. Hardware qualification remains limited to the PL70e-BT Bluetooth print on macOS described above.

The ARM64 Linux release archive also passed those packaging and extracted-executable checks locally in a disposable Alpine Linux container. No physical printer was contacted during the release smoke test.
