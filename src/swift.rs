use std::{path::PathBuf, process::Command};

use crate::{
    build::{Architecture, Platform},
    context::Ctx,
};

#[cfg(feature = "swift-bridge")]
pub struct SwiftCtx {
    /// The name of the Swift library to statically compile
    pub library_name: String,
    /// The path to the Swift library to statically compile
    pub library_path: PathBuf,
    /// The path to the Swift library's source code
    pub source_path: PathBuf,
    /// The path to the Swift library's build files
    pub build_path: PathBuf,
    /// The path to swift-bridge's `generated` folder
    pub generated_code_path: PathBuf,
    /// All of the "bridges" to target with swift-bridge
    pub bridges: Vec<PathBuf>,
}
impl SwiftCtx {
    pub fn new(ctx: &Ctx, release_mode: bool) -> Option<Self> {
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
                    let library_path = ctx.root_dir.join(swift_library_path);
                    let library_name = library_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string();
                    let source_path = library_path.join("Sources").join(&library_name);
                    let build_path = library_path.join(".build").join(if release_mode {
                        "release"
                    } else {
                        "debug"
                    });
                    let generated_code_path = source_path.join("generated");

                    Some(Self {
                        library_name,
                        library_path,
                        source_path,
                        build_path,
                        generated_code_path,
                        bridges,
                    })
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
}

/// Generates the static/unchanging arguments for Swift and Cargo (and returns them in that order)
#[cfg(feature = "swift-bridge")]
pub fn static_args(ctx: &mut Ctx, release_mode: bool) -> Option<(Vec<String>, Vec<String>)> {
    let swift_ctx = SwiftCtx::new(ctx, release_mode)?;

    // Arguments for the Swift compiler
    let bridging_header = swift_ctx.source_path.join("bridging-header.h");
    let mut swift_args = vec![
        "--package-path".to_string(),
        swift_ctx.library_path.to_str().unwrap().to_string(),
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
        "static=".to_string() + &swift_ctx.library_name,
        // Add search paths for linking to Swift libraries
        "-L".to_string(),
        swift_ctx.build_path.to_str().unwrap().to_string(),
    ];

    cargo_args.push("-L".to_string());
    cargo_args.push("/usr/lib/swift".to_string());

    // Let swift_bridge generate FFI for Rust <-> Swift
    swift_bridge_build::parse_bridges(swift_ctx.bridges)
        .write_all_concatenated(swift_ctx.generated_code_path, &ctx.project_id);

    // We need to force Cargo to recompile the Rust code, otherwise it won't
    // link to the updated Swift library
    ctx.force_cargo_recompile = true;

    Some((swift_args, cargo_args))
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
