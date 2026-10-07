fn main() {
    // `semantic_runtime` marks the targets that compile the ONNX runtime
    // behind `fastembed`. Keep in step with the fastembed target section in
    // Cargo.toml: Intel macOS has no prebuilt runtime.
    println!("cargo::rustc-check-cfg=cfg(semantic_runtime)");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let intel_mac = target_os == "macos" && target_arch == "x86_64";
    if !intel_mac {
        println!("cargo::rustc-cfg=semantic_runtime");
    }

    // The attribute tests the host that compiles this script, and the
    // runtime check the target, so a Mac cross-compiling for another OS does
    // not link the Swift packages.
    #[cfg(target_os = "macos")]
    if target_os == "macos" {
        use swift_rs::SwiftLinker;
        // Link Swift runtime and compile Swift sources
        SwiftLinker::new("10.15")
            .with_package("EventKitBridge", "./src-swift/")
            .with_package("MoldaviteCloud", "./src-swift-cloud/")
            .link();
    }

    tauri_build::build()
}
