use std::{path::PathBuf, process::Command};

use crate::{
    build::{Architecture, Platform},
    context::Ctx,
};

/// Generates the static/unchanging arguments for Swift and Cargo (and returns them in that order)
pub fn static_args(ctx: &Ctx, release_mode: bool) -> Option<(Vec<String>, Vec<String>)> {
    #[cfg(not(feature = "swift-bridge"))]
    return None;

    #[cfg(feature = "swift-bridge")]
    if let Some(cfg) = &ctx.cfg {
        if let Some(toml::Value::Array(bridges)) = cfg.get("swift-bridges") {
            if let Some(toml::Value::String(swift_library_path)) = cfg.get("swift-library") {
                // Convert the toml values to their correct Rust types
                let bridges: Vec<PathBuf> = bridges
                    .iter()
                    .map(|val| {
                        ctx.root_dir.join(
                            val.to_string()
                                .strip_prefix('"')
                                .unwrap()
                                .strip_suffix('"')
                                .unwrap(),
                        )
                    })
                    .collect();
                let swift_library_path = ctx.root_dir.join(swift_library_path);
                let swift_library_name = swift_library_path.file_name().unwrap().to_str().unwrap();
                let swift_source_path = swift_library_path.join("Sources").join(swift_library_name);
                let swift_build_path = swift_library_path.join(".build").join(if release_mode {
                    "release"
                } else {
                    "debug"
                });
                let generated_code_path = swift_source_path.join("generated");

                // Arguments for the Swift compiler
                let bridging_header = swift_source_path.join("bridging-header.h");
                let mut swift_args = vec![
                    "--package-path".to_string(),
                    swift_library_path.to_str().unwrap().to_string(),
                    "-Xswiftc".to_string(),
                    "-static".to_string(),
                    "-Xswiftc".to_string(),
                    "-import-objc-header".to_string(),
                    "-Xswiftc".to_string(),
                    bridging_header.to_str().unwrap().to_string(),
                ];
                if release_mode {
                    swift_args.push("-c".to_string());
                    swift_args.push("release".to_string());
                }

                // We'll add arguments for the Cargo command here, and return it later
                let mut cargo_args = vec![
                    // Link Rust to the Swift package
                    "--".to_string(),
                    "-l".to_string(),
                    "static=".to_string() + swift_library_name,
                    // Add search paths for linking to Swift libraries
                    "-L".to_string(),
                    swift_build_path.to_str().unwrap().to_string(),
                ];

                cargo_args.push("-L".to_string());
                cargo_args.push("/usr/lib/swift".to_string());

                // Let swift_bridge generate FFI for Rust <-> Swift
                swift_bridge_build::parse_bridges(bridges)
                    .write_all_concatenated(generated_code_path, &ctx.project_id);

                Some((swift_args, cargo_args))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

pub fn get_sdk(platform: Platform) -> String {
    let sdk = match platform {
        Platform::macOS => "macosx",
        Platform::iOS => "iphoneos",
    };
    let output = Command::new("xcrun")
        .arg("--sdk")
        .arg(sdk)
        .arg("--show-sdk-path")
        .output()
        .unwrap();
    String::from_utf8(output.stdout.as_slice().into())
        .unwrap()
        .trim()
        .to_string()
}

pub fn get_target_triple(platform: Platform, architecture: Architecture) -> String {
    String::from(match architecture {
        Architecture::x86_64 => "x86_64",
        Architecture::aarch64 => "arm64",
    }) + "-apple-"
        + match platform {
            Platform::iOS => "ios14",
            Platform::macOS => "macosx11",
        }
}
