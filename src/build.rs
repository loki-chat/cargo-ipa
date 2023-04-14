use clap::Args;
use std::{collections::HashMap, fs, process::Command};

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

pub fn build(args: BuildArgs) -> Result<(), String> {
    let ctx = &Ctx::new(args.release, args.name).unwrap();

    // ========== COMPILATION ==========
    println!("Compiling Rust binary...");
    // The arguments to pass to cargo
    let cargo_args = if args.release {
        vec!["build", "--target", TARGET_TRIPLE, "--release"]
    } else {
        vec!["build", "--target", TARGET_TRIPLE]
    };
    // Compile the project/example
    let build_status = if let Some(ref example_name) = args.example {
        Command::new("cargo")
            .args(cargo_args)
            .arg("--example")
            .arg(example_name)
            .status()
    } else {
        Command::new("cargo").args(cargo_args).status()
    };

    // Make sure building succeeded
    if build_status.is_err() || !build_status.unwrap().success() {
        return Err("Cargo failed to compile the project! Aborting.".into());
    }

    // ========== GENERATE IPA ==========
    println!("Generating app...");
    println!("|- Copying the binary...");
    // The binary's name & location will change if we're compiling an example or a binary package
    let (bin_dir, binary_name) = if let Some(ref example_name) = args.example {
        (&ctx.examples_dir, example_name.to_owned())
    } else {
        (&ctx.build_dir, ctx.project_id.to_owned())
    };
    let binary = fs::read(bin_dir.join(&binary_name));
    // Make sure reading the binary & copying it succeeded
    if binary.is_err() || fs::write(ctx.app_dir.join(&binary_name), binary.unwrap()).is_err() {
        return Err("Failed to copy the binary into the app! Aborting.".into());
    }
    println!("|- Generating `Info.plist`...");
    // A map of the Info.plist values, and some default necessary values
    let mut map = HashMap::<String, String>::new();
    map.insert("CFBundleExecutable".into(), binary_name);
    map.insert(
        "CFBundleIdentifier".into(),
        "com.".to_owned() + ctx.project_id.as_str(),
    );
    map.insert("CFBundleName".into(), ctx.project_name.clone());
    map.insert("CFBundleVersion".into(), ctx.project_version.clone());
    map.insert(
        "CFBundleShortVersionString".into(),
        ctx.project_version.clone(),
    );
    // Write everything to Info.plist & make sure it succeeds
    if let Err(e) = fs::write(ctx.app_dir.join("Info.plist"), gen_info_plist(map)) {
        return Err(format!(
            "Failed to write to your app's Info.plist! The error was: {e} Aborting."
        ));
    }

    println!("Compressing app into an IPA...");
    let ipa_file = ctx.build_dir.join(ctx.project_name.clone() + ".ipa");
    if ipa_file.is_file() {
        if let Err(e) = fs::remove_file(&ipa_file) {
            panic!("Failed to create IPA file: {e}");
        }
    }

    // Need to go to relative path above Payload - otherwise the path is weird in the zip file
    // (eg /full/path/to/Payload instead of Payload)
    std::env::set_current_dir(&ctx.build_dir).expect("Failed to go to build directory");

    let zip_cmd = Command::new("zip")
        .arg("-r")
        .arg(ctx.project_name.clone() + ".ipa")
        .arg("Payload")
        .status();

    if zip_cmd.is_err() || !zip_cmd.unwrap().success() {
        return Err("Failed to compress your app into an IPA! Aborting.".into());
    }

    // ========== CLEANUP ==========
    println!("Cleaning up...");
    if fs::remove_dir_all(&ctx.payload_dir).is_err() {
        return Err(format!("Failed to clean build files. You have an IPA file at {}, but future builds may fail due to conflicting build files.", ipa_file.to_str().unwrap()));
    }

    println!("Done! IPA is at {}", ipa_file.to_str().unwrap());
    Ok(())
}

/// Generate the Info.plist file
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
