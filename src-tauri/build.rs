use std::{
    env,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

fn same_contents(source: &Path, destination: &Path) -> std::io::Result<bool> {
    let mut left = std::fs::File::open(source)?;
    let mut right = match std::fs::File::open(destination) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let a = left.metadata()?;
    let b = right.metadata()?;
    if a.len() != b.len() || a.permissions() != b.permissions() {
        return Ok(false);
    }
    let mut left_bytes = [0; 64 * 1024];
    let mut right_bytes = [0; 64 * 1024];
    loop {
        let count = left.read(&mut left_bytes)?;
        if count == 0 {
            return Ok(true);
        }
        right.read_exact(&mut right_bytes[..count])?;
        if left_bytes[..count] != right_bytes[..count] {
            return Ok(false);
        }
    }
}
fn copy_if_changed(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> std::io::Result<()> {
    if !same_contents(source.as_ref(), destination.as_ref())? {
        std::fs::copy(source, destination)?;
    }
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=native/media.swift");
    println!("cargo:rerun-if-changed=native/photo.swift");
    println!("cargo:rerun-if-changed=native/pdf.swift");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        assert_eq!(
            env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
            Ok("aarch64"),
            "The bundled Whisper helper currently targets Apple Silicon arm64 builds."
        );
        let manifest_dir = PathBuf::from(
            env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
        );
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
        let status = Command::new("sh")
            .arg("scripts/provision-audio-assets.sh")
            .current_dir(&manifest_dir)
            .status()
            .expect("The pinned local Whisper ASR provisioning script must start");
        assert!(
            status.success(),
            "Pinned local Whisper ASR assets could not be provisioned"
        );
        let whisper_helper = manifest_dir
            .join("resources/audio/whisper-cli")
            .canonicalize()
            .expect("The provisioned Whisper helper must exist");
        println!(
            "cargo:rustc-env=KG_WHISPER_HELPER_BIN={}",
            whisper_helper.display()
        );
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
        let photo_helper = out.join("photo-helper");
        let photo_status = Command::new("/usr/bin/swiftc")
            .args(["-O", "native/photo.swift", "-o"])
            .arg(photo_helper)
            .status()
            .expect("The native photo helper requires the macOS Swift compiler at build time.");
        assert!(
            photo_status.success(),
            "Native photo helper compilation failed."
        );
        let pdf_status = Command::new("/usr/bin/swiftc")
            .args(["-O", "-framework", "PDFKit", "native/pdf.swift", "-o"])
            .arg(out.join("pdf-helper"))
            .status()
            .expect("The native PDF helper requires the macOS Swift compiler at build time.");
        assert!(
            pdf_status.success(),
            "Native PDF helper compilation failed."
        );
    }
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let helper = manifest_dir.join("container-helper");
        println!("cargo:rerun-if-changed=src/containers.rs");
        println!("cargo:rerun-if-changed=container-helper/src/main.rs");
        println!("cargo:rerun-if-changed=container-helper/Cargo.lock");
        let profile = env::var("PROFILE").unwrap_or_default();
        let mut build = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        build
            .arg("build")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(helper.join("Cargo.toml"));
        if profile == "release" {
            build.arg("--release");
        }
        build.env_remove("CARGO_TARGET_DIR");
        assert!(
            build
                .status()
                .expect("container helper build must start")
                .success(),
            "container helper build failed"
        );
        let triple = env::var("TARGET").unwrap();
        let binaries = manifest_dir.join("binaries");
        std::fs::create_dir_all(&binaries).unwrap();
        copy_if_changed(
            helper
                .join("target")
                .join(if profile == "release" {
                    "release"
                } else {
                    "debug"
                })
                .join("kg-container-helper"),
            binaries.join(format!("kg-container-helper-{triple}")),
        )
        .expect("container helper must be bundled");
        println!("cargo:rerun-if-changed=container-helper/gmime_unit.c");
        println!("cargo:rerun-if-changed=container-helper/provision_gmime.py");
        println!("cargo:rerun-if-changed=container-helper/gmime-runtime-lock.json");
        println!("cargo:rerun-if-changed=vendor/gmime/manifest.json");
        println!("cargo:rerun-if-changed=vendor/gmime/gmime-3.2.15-arm64.tar.gz");
        let generated = helper.join("generated-gmime");
        assert!(
            Command::new("python3")
                .arg(helper.join("provision_gmime.py"))
                .arg(&generated)
                .status()
                .expect("GMime provisioning must start")
                .success(),
            "Pinned GMime runtime provisioning failed"
        );
        copy_if_changed(
            generated.join("kg-mime-unit"),
            binaries.join(format!("kg-mime-unit-{triple}")),
        )
        .expect("GMime helper must be bundled");
        for name in ["container-runtime", "container-licenses"] {
            let destination = manifest_dir.join("resources").join(name);
            std::fs::create_dir_all(&destination).unwrap();
            for entry in std::fs::read_dir(generated.join(name)).unwrap() {
                let entry = entry.unwrap();
                let target = destination.join(entry.file_name());
                copy_if_changed(entry.path(), target).unwrap();
            }
        }
    }
    tauri_build::build();
}
