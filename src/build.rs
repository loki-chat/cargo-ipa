use clap::{Args, ValueEnum};
use std::{collections::HashMap, fs, process::Command};

use crate::{context::*, Ctx};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum Platform {
    #[value(rename_all = "verbatim")]
    macOS,
    #[value(rename_all = "verbatim")]
    iOS,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum Architecture {
    #[value(rename_all = "verbatim")]
    x86_64,
    aarch64,
}

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
    /// Only compile for 1 platform instead of both
    #[arg(short, long, value_enum)]
    platform: Option<Platform>,
    /// Only compile for 1 architecture instead of both
    #[arg(short, long, value_enum)]
    architecture: Option<Architecture>,
}

pub fn build(args: BuildArgs) -> Result<(), String> {
    let ctx = &Ctx::new(&args.name).unwrap();

    // ========== SETUP ==========
    println!("Setting up...");
    // These arguments to Cargo will never change, since they don't rely on target triples
    let mut static_cargo_args = Vec::new();
    if args.release {
        static_cargo_args.push("--release".to_string());
    }
    if let Some(ref example_name) = args.example {
        static_cargo_args.push("--example".to_string());
        static_cargo_args.push(example_name.to_string());
    }
    if let Ok(args) = crate::swift::compile_swift(ctx, args.release) {
        static_cargo_args.extend(args.into_iter());
    }
    let binary_name = if let Some(ref example_name) = args.example {
        example_name.to_string()
    } else {
        ctx.project_id.to_string()
    };

    // ========== GENERATE INFO.PLIST ==========
    println!("Generating `Info.plist`...");
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
    // Check for Info.plist overrides in Cargo.toml
    if let Some(cfg) = &ctx.cfg {
        if let Some(toml::Value::Table(properties)) = cfg.get("properties") {
            for (key, value) in properties.into_iter() {
                map.insert(key.to_owned(), value.to_string());
            }
        }
    }
    // Write everything to Info.plist & make sure it succeeds
    if let Err(e) = fs::write(ctx.cargo_ipa_dir.join("Info.plist"), gen_info_plist(map)) {
        return Err(format!(
            "Failed to write to Info.plist! The error was: {e}\nAborting."
        ));
    }

    // ========== COMPILATION ==========
    for (platform, target_triple) in gen_targets_list(&args) {
        println!("Compiling for {target_triple}...");
        // Generate the arguments list for Cargo
        let mut cargo_args = vec!["rustc", "--target", &target_triple, "-q"];
        cargo_args.extend(static_cargo_args.iter().map(|item| item.as_str()));

        let build_status = Command::new("cargo").args(cargo_args).status();

        // Make sure building succeeded
        if build_status.is_err() || !build_status.unwrap().success() {
            return Err("Cargo failed to compile the project! Aborting.".into());
        }

        // Make the .ipa or .app file, as appropriate
        match platform {
            Platform::macOS => gen_app(ctx, &target_triple, &args)?,
            Platform::iOS => gen_ipa(ctx, &target_triple, &args)?,
        };
    }

    // ========== CLEANUP ==========
    println!("Cleaning up...");

    println!(
        "Done! Your build files are at `{}`",
        ctx.cargo_ipa_dir.to_str().unwrap()
    );
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

/// Generate a list of targets to compile for
fn gen_targets_list(args: &BuildArgs) -> Vec<(Platform, String)> {
    // Cache the architectures being used
    let architectures = if let Some(architecture) = args.architecture {
        match architecture {
            Architecture::aarch64 => vec!["aarch64"],
            Architecture::x86_64 => vec!["x86_64"],
        }
    } else {
        vec!["aarch64", "x86_64"]
    };

    // Cache the platforms being used
    let platforms = if let Some(platform) = args.platform {
        match platform {
            Platform::iOS => vec![(Platform::iOS, "ios")],
            Platform::macOS => vec![(Platform::macOS, "darwin")],
        }
    } else {
        vec![(Platform::iOS, "ios"), (Platform::macOS, "darwin")]
    };

    // Merge the two into result
    let mut result = Vec::new();
    for architecture in architectures {
        for platform in &platforms {
            // Generate the target triple from the architecture and platform
            result.push((
                platform.0,
                architecture.to_string() + "-apple-" + platform.1,
            ));
        }
    }

    result
}

/// Compress everything into an IPA file
fn gen_ipa(ctx: &Ctx, target_triple: &str, args: &BuildArgs) -> Result<String, String> {
    // Make sure the IPA file doesn't already exist;
    // otherwise, the zip command will add to it instead of making a new one
    let ipa_file = ctx
        .cargo_ipa_dir
        .join(ctx.project_name.clone() + target_triple + ".ipa");
    if ipa_file.exists() {
        if let Err(e) = fs::remove_file(&ipa_file) {
            return Err(
                "Error: IPA file already exists, and can't be removed: ".to_owned()
                    + &e.to_string(),
            );
        }
    }

    // Make a new Payload folder inside our build directory
    let payload_folder = ctx.cargo_ipa_dir.join("Payload");
    if payload_folder.exists() {
        if let Err(e) = fs::remove_dir_all(&payload_folder) {
            return Err(
                "Error: Build files already exist, and can't be removed: ".to_owned()
                    + &e.to_string(),
            );
        }
    }
    if let Err(e) = fs::create_dir(&payload_folder) {
        return Err("Error: Failed to create build directory: ".to_owned() + &e.to_string());
    }

    // TODO: Make a .app folder, and put it inside the Payload directory
    let app_name = gen_app(ctx, target_triple, args)?;
    println!("|- Compressing the app into an IPA...");
    println!(
        "Moving {} from {} to {}",
        &app_name,
        ctx.cargo_ipa_dir.join(&app_name).to_str().unwrap(),
        payload_folder.join(&app_name).to_str().unwrap()
    );
    if let Err(e) = fs::rename(
        ctx.cargo_ipa_dir.join(&app_name),
        payload_folder.join(&app_name),
    ) {
        return Err(
            "Error: Failed to copy .app file for compression: ".to_string() + &e.to_string(),
        );
    }

    // Need to go to relative path above Payload - otherwise the path is weird in the zip file
    // (eg /full/path/to/Payload instead of Payload)
    std::env::set_current_dir(&ctx.cargo_ipa_dir).expect("Failed to go to build directory");

    // Zip the Payload folder into our ipa file
    let zip_cmd = Command::new("zip")
        .arg("-r")
        .arg(ctx.project_name.clone() + ".ipa")
        .arg("Payload")
        .status();

    if zip_cmd.is_err() || !zip_cmd.unwrap().success() {
        return Err("Error: Failed to compress the app into an IPA! Aborting.".into());
    }

    Ok(ipa_file.to_str().unwrap().to_string())
}

/// Compress everything into an .app file
fn gen_app(ctx: &Ctx, target_triple: &str, args: &BuildArgs) -> Result<String, String> {
    println!("|- Generating .app file...");
    // Where the .app folder will be placed
    let app_name = ctx.project_name.clone() + "." + target_triple + ".app";
    let app_path = ctx.cargo_ipa_dir.join(&app_name);
    if app_path.exists() {
        if let Err(e) = fs::remove_dir_all(&app_path) {
            return Err(
                "Error: App file already exists, and can't be removed: ".to_owned()
                    + &e.to_string(),
            );
        }
    }
    if let Err(e) = fs::create_dir(&app_path) {
        return Err("Error: Failed to create .app directory: ".to_owned() + &e.to_string());
    }

    // Find the binary
    let bin_name = if let Some(ref example_name) = args.example {
        example_name
    } else {
        &ctx.project_id
    };
    let mut bin_path =
        ctx.target_dir
            .join(target_triple)
            .join(if args.release { "release" } else { "debug" });
    if args.example.is_some() {
        bin_path.push("examples");
    }
    bin_path.push(bin_name);

    // Find Info.plist
    let info_plist_path = ctx.cargo_ipa_dir.join("Info.plist");

    println!("   |- Copying Info.plist...");
    let info_plist = fs::read(info_plist_path);
    if info_plist.is_err() || fs::write(app_path.join("Info.plist"), info_plist.unwrap()).is_err() {
        return Err("Error: Failed to copy Info.plist to the new app".into());
    }
    println!("   |- Copying the binary...");
    let binary = fs::read(bin_path);
    if binary.is_err() || fs::write(app_path.join(bin_name), binary.unwrap()).is_err() {
        return Err("Error: Failed to copy the binary to the new app".into());
    }

    Ok(app_name)
}
