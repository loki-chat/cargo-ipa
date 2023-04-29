use std::{path::PathBuf, process::Command};

use crate::context::Ctx;

pub fn compile_swift(ctx: &Ctx, release_mode: bool) -> Result<Vec<String>, ()> {
    #[cfg(not(feature = "swift-bridge"))]
    return Err(());

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
                                .unwrap()
                                .to_string(),
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

                // We'll add arguments for the Cargo command here, and return it later
                let mut cargo_args = Vec::new();

                // Arguments for the Swift compiler
                let bridging_header = swift_source_path.join("bridging-header.h");
                let mut swift_args = vec![
                    "build",
                    "-Xswiftc",
                    "-static",
                    "-Xswiftc",
                    "-import-objc-header",
                    "-Xswiftc",
                    bridging_header.to_str().unwrap(),
                ];
                if release_mode {
                    swift_args.push("-c");
                    swift_args.push("release");
                }

                // Let swift_bridge generate FFI for Rust <-> Swift
                swift_bridge_build::parse_bridges(bridges)
                    .write_all_concatenated(generated_code_path, &ctx.project_id);

                // Attempt to compile the Swift package
                let build_status = Command::new("swift")
                    .current_dir(&swift_library_path)
                    .args(swift_args)
                    .status();
                if build_status.is_err() || !build_status.unwrap().success() {
                    println!("Swift failed to compile the project! Attempting to compile without the Swift library...");
                    return Err(());
                }

                // Link Rust to the Swift package
                cargo_args.push("--".to_string());
                cargo_args.push("-l".to_string());
                cargo_args.push("static=".to_string() + swift_library_name);
                cargo_args.push("-l".to_string());
                cargo_args.push("static=".to_string() + swift_library_name);

                // Add search paths for linking to Swift libraries
                cargo_args.push("-L".to_string());
                cargo_args.push(swift_build_path.to_str().unwrap().to_string());
                let xcode_path = if let Ok(output) = std::process::Command::new("xcode-select")
                    .arg("--print-path")
                    .output()
                {
                    String::from_utf8(output.stdout.as_slice().into())
                        .unwrap()
                        .trim()
                        .to_string()
                } else {
                    "/Applications/Xcode.app/Contents/Developer".to_string()
                };
                cargo_args.push("-L".to_string());
                cargo_args.push(
                    xcode_path + "/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx/",
                );
                cargo_args.push("-L".to_string());
                cargo_args.push("/usr/lib/swift".to_string());

                Ok(cargo_args)
            } else {
                println!("Swift bridges were listed, but no Swift package was listed to compile!");
                Err(())
            }
        } else {
            Err(())
        }
    } else {
        Err(())
    }
}
