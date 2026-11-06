use std::process::Command;

#[test]
fn reports_the_new_command_name_and_project_notice() {
    let version = Command::new(env!("CARGO_BIN_EXE_open-nelko"))
        .arg("--version")
        .output()
        .expect("CLI runs");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("open-nelko "));
    let help = Command::new(env!("CARGO_BIN_EXE_open-nelko"))
        .arg("--help")
        .output()
        .expect("CLI runs");
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("unaffiliated with Nelko"));
    assert!(text.contains("MIT licensed"));
}

#[test]
fn requires_explicit_transport_before_printing() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-nelko"))
        .args(["print", "label.pdf"])
        .output()
        .expect("CLI runs");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--transport"));
}

#[test]
fn rejects_zero_copies_before_accessing_file_or_hardware() {
    let output = Command::new(env!("CARGO_BIN_EXE_open-nelko"))
        .args(["print", "label.pdf", "--transport", "usb", "--copies", "0"])
        .output()
        .expect("CLI runs");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid value"));
}
