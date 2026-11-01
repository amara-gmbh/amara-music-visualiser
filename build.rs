use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=tools/amaraAudioCapture/main.swift");
    println!("cargo:rerun-if-changed=packaging/Info.plist");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let helper = out_dir.join("amaraAudioCapture");
    let module_cache = out_dir.join("swift-module-cache");
    let architecture = env::var("CARGO_CFG_TARGET_ARCH").expect("target architecture");
    let swift_architecture = if architecture == "aarch64" {
        "arm64"
    } else {
        "x86_64"
    };
    let deployment_target = format!("{swift_architecture}-apple-macosx13.0");
    let info_plist = env::current_dir()
        .expect("project directory")
        .join("packaging/Info.plist");
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-parse-as-library",
            "-module-cache-path",
            module_cache.to_str().expect("Swift module cache path"),
            "-target",
            &deployment_target,
            "tools/amaraAudioCapture/main.swift",
            "-O",
            "-framework",
            "Foundation",
            "-framework",
            "ScreenCaptureKit",
            "-framework",
            "CoreMedia",
            "-framework",
            "CoreAudio",
            "-framework",
            "CoreGraphics",
        ])
        .args([
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__info_plist",
            "-Xlinker",
        ])
        .arg(info_plist)
        .arg("-o")
        .arg(&helper)
        .status();

    match status {
        Ok(status) if status.success() => {
            println!(
                "cargo:rustc-env=AMARA_AUDIO_HELPER_BUILD_PATH={}",
                helper.display()
            );
        }
        Ok(status) => println!(
            "cargo:warning=System-audio helper did not compile (swiftc exit {status}); microphone and demo modes remain available"
        ),
        Err(error) => println!(
            "cargo:warning=Could not run swiftc for system-audio helper ({error}); microphone and demo modes remain available"
        ),
    }
}
