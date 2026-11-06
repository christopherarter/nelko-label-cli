# Hardware qualification

Initial target: Nelko PL70e-BT, 203 dpi, 4×6 thermal labels, on macOS.

| Check | Status |
| --- | --- |
| Printer language | Installed PL70e-BT driver contains TSPL SIZE, BITMAP and PRINT commands |
| PDF rasterization | Synthetic orientation/page-selection tests pass; actual 4×6 label preview visually checked |
| USB enumeration and interface claim | Awaiting connected hardware test |
| USB physical print | Unverified |
| Bluetooth discovery and services | Verified; FFF0/FFF2 channel answered TSPL status with 0x00 (ready) |
| Bluetooth physical print | Verified 2026-11-06; user confirmed the requested PDF printed correctly at full 4×6 size |
| Intel Mac executable | Cross-compiled release target; not hardware tested |
| Linux PDF/CLI path | All 10 synthetic tests pass in a native ARM64 musl Linux container; physical printer access on Linux is unverified |

Support for both transport APIs is implemented. Physical print compatibility must be confirmed independently for the exact printer revision. A successful byte transfer does not establish that the printer interpreted or printed the job.

Report the printer model, firmware revision if known, OS version, CLI version, connection type and the visible result. Do not attach real address labels or tracking barcodes. Use a synthetic test label.
