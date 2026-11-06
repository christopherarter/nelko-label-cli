use std::process::Command;

#[test]
fn requires_explicit_transport_before_printing() {
    let output = Command::new(env!("CARGO_BIN_EXE_nelko"))
        .args(["print", "label.pdf"])
        .output()
        .expect("CLI runs");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--transport"));
}

#[test]
fn rejects_zero_copies_before_accessing_file_or_hardware() {
    let output = Command::new(env!("CARGO_BIN_EXE_nelko"))
        .args(["print", "label.pdf", "--transport", "usb", "--copies", "0"])
        .output()
        .expect("CLI runs");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
}
