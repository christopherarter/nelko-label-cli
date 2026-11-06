use std::{env, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=resources/Info.plist");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let manifest =
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets manifest dir"));
        println!(
            "cargo:rustc-link-arg=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            manifest.join("resources/Info.plist").display()
        );
    }
}
