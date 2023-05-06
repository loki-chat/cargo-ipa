mod context;
mod swift;

use {
    context::*,
    std::{env, process::Command},
    swift::SwiftCtx,
};

/// Uses swift-bridge to generate FFI bindings between Swift and Rust
#[cfg(feature = "swift-bridge")]
pub fn generate_bindings() -> Result<(), String> {
    let ctx = Ctx::new(&None)?;
    let swift_ctx = SwiftCtx::new(&ctx, release_mode())?;
    swift_bridge_build::parse_bridges(swift_ctx.bridges)
        .write_all_concatenated(swift_ctx.generated_code_path, &ctx.project_id);

    Ok(())
}

/// Compiles the Swift package, and tells Cargo to statically link it
pub fn compile_and_link_swift() -> Result<(), String> {
    // Setup
    let release_mode = release_mode();
    let ctx = Ctx::new(&None)?;
    let swift_ctx = SwiftCtx::new(&ctx, release_mode)?;
    let static_swift_args = swift::static_swiftc_args(&swift_ctx, release_mode);
    let target_triple = env::var("TARGET")
        .unwrap()
        .replace("aarch64", "arm64") // Map the Rust target triple to a Swift target triple
        .replace("ios", "ios14")
        .replace("darwin", "macosx11");
    let platform = if target_triple.contains("ios") {
        Platform::iOS
    } else {
        Platform::macOS
    };
    let sdk = swift::get_sdk(platform);

    // Compile the Swift package
    let mut swift_args = vec![
        "build",
        "-Xswiftc",
        "-target",
        "-Xswiftc",
        &target_triple,
        "--sdk",
        &sdk,
    ];
    swift_args.extend(static_swift_args.iter().map(|item| item.as_str()));
    let build_status = Command::new("swift")
        .args(swift_args)
        .spawn()
        .unwrap()
        .wait_with_output()
        .unwrap();
    if !build_status.status.success() {
        return Err(format!(
            "Swift failed to compile the project! Stdout:\n{}\n\nStderr:\n{}",
            String::from_utf8(build_status.stderr).unwrap(),
            String::from_utf8(build_status.stdout).unwrap()
        ));
    }

    // Tell Cargo to statically link the Swift package
    println!("cargo:rustc-link-lib=static={}", &swift_ctx.library_name);
    println!(
        "cargo:rustc-link-search={}",
        swift_ctx.build_path.to_str().unwrap()
    );
    println!(
        "cargo:rustc-link-search={}",
        detect_xcode()
            .join(match platform {
                Platform::macOS => "macosx",
                Platform::iOS => "iphoneos",
            })
            .to_str()
            .unwrap()
    );
    println!("cargo:rustc-link-search=/usr/lib/swift");

    Ok(())
}

/// Checks Cargo's PROFILE env variable to see if we're building in release mode
fn release_mode() -> bool {
    env::var("PROFILE").unwrap() == "release"
}
