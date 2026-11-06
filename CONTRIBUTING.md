# Contributing

Use Rust 1.89 or later. macOS PDF tests need Apple's command line developer tools.

Run formatting, Clippy with warnings as errors, and tests before opening a pull request. New transport work should include synthetic payload tests and an entry in HARDWARE.md distinguishing simulated results, transferred bytes and observed physical prints.

Keep user shipping labels out of the repository, test fixtures, logs and issue attachments. The test suite generates its own asymmetric PDF to check orientation, page selection and white backgrounds.

Releases are built from committed Cargo.lock dependencies. Update the package version and embedded Info.plist version together, then tag `v<version>`. The release workflow creates a draft for review.
