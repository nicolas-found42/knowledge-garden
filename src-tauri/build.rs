use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/media.swift");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
        let helper = out.join("media-helper");
        let status = Command::new("/usr/bin/swiftc")
            .args([
                "-parse-as-library",
                "-O",
                "-framework",
                "AVFoundation",
                "-framework",
                "Speech",
                "native/media.swift",
                "-o",
            ])
            .arg(&helper)
            .status()
            .expect("Swift compiler must be available to build the macOS media helper");
        assert!(status.success(), "native media helper compilation failed");
        println!("cargo:rustc-env=KG_MEDIA_HELPER_BIN={}", helper.display());
    }
    tauri_build::build();
}
