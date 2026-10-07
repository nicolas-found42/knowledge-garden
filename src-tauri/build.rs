fn main() {
    println!("cargo:rerun-if-changed=native/photo.swift");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let output =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("photo-helper");
        let result = std::process::Command::new("/usr/bin/swiftc")
            .args(["-O", "native/photo.swift", "-o"])
            .arg(output)
            .status()
            .expect("The native photo helper requires the macOS Swift compiler at build time.");
        assert!(result.success(), "Native photo helper compilation failed.");
    }
    tauri_build::build();
}
