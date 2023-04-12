use clap::Args;
use std::{collections::HashMap, fs, process::Command};
use toml::Value;

use crate::{context::*, Ctx};

#[derive(Args)]
pub struct BuildArgs {
    /// Compile the provided library example into an IPA.
    /// If blank, will compile the Rust binary.
    #[arg(short, long)]
    example: Option<String>,
    /// Compile in release mode
    #[arg(short, long)]
    release: bool,
    /// The app's name. If left unprovided, cargo-ipa will search
    /// for it in Cargo.toml. If it can't find it there, it will
    /// crash.
    #[arg(short, long)]
    name: Option<String>,
}

pub fn build(args: BuildArgs, ctx: &Ctx) {
    let BuildArgs {
        example,
        release,
        name,
    } = args;

    // Get the app name, either from Cargo.toml overrides
    let name = match name {
        Some(name) => name,
        None => match &ctx.cfg {
            Some(cfg) => match cfg.get("CFBundleName") {
                Some(val) => {
                    if let Value::String(name) = val {
                        name.to_owned()
                    } else {
                        panic!("Failed to find the app name! Invalid type for `CFBundleName` in Cargo.toml.")
                    }
                }
                None => panic!("Failed to find the app name! Please provide a value in Cargo.toml or pass the `name` argument."),
            },
            None => panic!("Failed to find the app name! Please provide a value in Cargo.toml or pass the `name` argument."),
        },
    };
    let build_dir = ctx.build_dir.join(name.clone() + ".app");
    if let Err(e) = fs::create_dir(&build_dir) {
        panic!("Failed to create build directory: {e}");
    }

    println!("Compiling Rust binary...");
    // The arguments to pass to cargo
    let cargo_args = if release {
        vec!["build", "--target", TARGET, "--release"]
    } else {
        vec!["build", "--target", TARGET]
    };
    // Compile the project/example
    let build_status = if let Some(ref example_name) = example {
        Command::new("cargo")
            .args(cargo_args)
            .arg("--example")
            .arg(example_name)
            .status()
    } else {
        Command::new("cargo").args(cargo_args).status()
    };
    if build_status.is_err() {
        panic!("Build failed, aborting...");
    }

    println!("Generating app...");
    println!("|- Copying the binary...");
    let subdir = if release { "release" } else { "debug" };
    let (bin_dir, binary_name) = if let Some(ref example_name) = example {
        (
            ctx.bin_dir.join(subdir).join("examples"),
            example_name.to_owned(),
        )
    } else {
        (ctx.bin_dir.join(subdir), ctx.project_name.to_owned())
    };
    if let Err(e) = fs::write(
        build_dir.join(&binary_name),
        fs::read(bin_dir.join(&binary_name)).expect("Failed to find compiled binary!"),
    ) {
        panic!("Failed to copy the binary: {e}");
    }
    println!("|- Generating `Info.plist`...");
    // A map of the Info.plist values, and some default necessary values
    let mut map = HashMap::<String, String>::new();
    map.insert("CFBundleExecutable".into(), binary_name);
    map.insert(
        "CFBundleIdentifier".into(),
        "com.".to_owned() + ctx.project_name.as_str(),
    );
    map.insert("CFBundleName".into(), name.clone());
    map.insert("CFBundleVersion".into(), ctx.project_version.clone());
    map.insert(
        "CFBundleShortVersionString".into(),
        ctx.project_version.clone(),
    );
    if let Err(e) = fs::write(build_dir.join("Info.plist"), gen_info_plist(map)) {
        panic!("Failed to write `Info.plist`: {e}");
    }

    println!("Compressing app into an IPA...");
    let ipa_file = ctx.build_dir.parent().unwrap().join(name.clone() + ".ipa");
    if ipa_file.is_file() {
        if let Err(e) = fs::remove_file(&ipa_file) {
            panic!("Failed to create IPA file: {e}");
        }
    }

    // Need to go to relative path above Payload - otherwise the path is weird in the zip file
    // (eg /full/path/to/Payload instead of Payload)
    std::env::set_current_dir(ctx.build_dir.parent().unwrap())
        .expect("Failed to go to build directory");

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let zip_cmd = Command::new("zip")
        .arg("-r")
        .arg(name.clone() + ".ipa")
        .arg("Payloard")
        .status();
    #[cfg(target_os = "windows")]
    let zip_cmd = Command::new("powershell")
        .arg("Compress-Archive")
        .arg("Payload")
        .arg(name.clone() + ".ipa")
        .status();

    match zip_cmd {
        Ok(status) => {
            if !status.success() {
                panic!("Zip command exited unsuccessfully");
            }
        }
        Err(e) => panic!("Zip command failed to run: {e}"),
    }

    println!("Cleaning up...");
    if let Err(e) = fs::remove_dir_all(&ctx.build_dir) {
        panic!("Failed to clean old build files: {e}");
    }

    println!(
        "Done! IPA is at {}",
        ctx.build_dir
            .parent()
            .unwrap()
            .join(name + ".ipa")
            .to_str()
            .unwrap()
    );
}

fn gen_info_plist(map: HashMap<String, String>) -> String {
    let mut buffer = String::new();
    buffer += PLIST_OPENING;

    for (key, value) in map.iter() {
        buffer += &format!("<key>{key}</key>\n");
        buffer += &format!("<string>{value}</string>\n");
    }

    buffer += PLIST_CLOSING;
    buffer
}
