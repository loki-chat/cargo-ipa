use clap::{Args, ValueEnum};
use std::{collections::HashMap, fs, path::PathBuf, process::Command};

use crate::{context::*, swift, Ctx};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum Platform {
    #[value(rename_all = "lower")]
    macOS,
    #[value(rename_all = "lower")]
    iOS,
}
impl ToString for Platform {
    fn to_string(&self) -> String {
        match self {
            Self::iOS => String::from("ios"),
            Self::macOS => String::from("darwin"),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[allow(non_camel_case_types)]
pub enum Architecture {
    #[value(rename_all = "verbatim")]
    x86_64,
    aarch64,
}
impl ToString for Architecture {
    fn to_string(&self) -> String {
        match self {
            Self::x86_64 => String::from("x86_64"),
            Self::aarch64 => String::from("aarch64"),
        }
    }
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
    let ctx = &mut Ctx::new(&args.name).unwrap();

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
    let static_swift_args =
        if let Some((swift_args, cargo_args)) = swift::static_args(ctx, args.release) {
            static_cargo_args.extend(cargo_args.into_iter());
            Some(swift_args)
        } else {
            None
        };
    let binary_name = if let Some(ref example_name) = args.example {
        example_name.to_string()
    } else {
        ctx.project_id.to_string()
    };
    // Find XCode Toolchain
    let mut xcode_toolchain = PathBuf::from(
        if let Ok(output) = std::process::Command::new("xcode-select")
            .arg("--print-path")
            .output()
        {
            String::from_utf8(output.stdout.as_slice().into())
                .unwrap()
                .trim()
                .to_string()
        } else {
            "/Applications/Xcode.app/Contents/Developer".to_string()
        },
    );
    xcode_toolchain.push("Toolchains/XcodeDefault.xctoolchain/usr/lib/swift");

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
    map.insert("CFBundlePackageType".to_string(), "APPL".to_string());
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
    for (platform, architecture) in gen_targets_list(&args) {
        let target_triple = architecture.to_string() + "-apple-" + &platform.to_string();
        println!("Compiling for {target_triple}...");

        if ctx.force_cargo_recompile {
            let mut cargo_args = vec!["clean", "-p", &ctx.project_id, "--target", &target_triple];

            if args.release {
                cargo_args.push("-r");
            }

            let clean_result = Command::new("cargo").args(cargo_args).status();
            if clean_result.is_err() || !clean_result.unwrap().success() {
                return Err("Failed to clean old build files.".to_string());
            }
        }

        // Compile Swift
        if let Some(ref static_swift_args) = static_swift_args {
            let target = swift::get_target_triple(platform, architecture);
            let sdk = swift::get_sdk(platform);
            let mut swift_args = vec![
                "build", "-Xswiftc", "-target", "-Xswiftc", &target, "--sdk", &sdk,
            ];
            swift_args.extend(static_swift_args.iter().map(|item| item.as_str()));

            let build_status = Command::new("swift").args(swift_args).status();
            if build_status.is_err() || !build_status.unwrap().success() {
                return Err("Swift failed to compile the project! Aborting.".into());
            }
        }

        // Compile Rust
        let mut cargo_args = vec!["rustc", "--target", &target_triple, "-q"];
        cargo_args.extend(static_cargo_args.iter().map(|item| item.as_str()));
        if !cargo_args.contains(&"--") {
            cargo_args.push("--");
        }
        cargo_args.push("-L");
        let platform_toolchain = xcode_toolchain.join(match platform {
            Platform::macOS => "macosx",
            Platform::iOS => "iphoneos",
        });
        cargo_args.push(platform_toolchain.to_str().unwrap());

        // Make sure building succeeded
        let build_status = Command::new("cargo").args(cargo_args).status();
        if build_status.is_err() || !build_status.unwrap().success() {
            return Err("Cargo failed to compile the project! Aborting.".into());
        }

        // Make the .ipa or .app file, as appropriate
        match platform {
            Platform::macOS => gen_app(ctx, &target_triple, &args, true)?,
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
fn gen_targets_list(args: &BuildArgs) -> Vec<(Platform, Architecture)> {
    // Cache the architectures being used
    let architectures = if let Some(architecture) = args.architecture {
        vec![architecture]
    } else {
        vec![Architecture::x86_64, Architecture::aarch64]
    };

    // Cache the platforms being used
    let platforms = if let Some(platform) = args.platform {
        vec![platform]
    } else {
        vec![Platform::iOS, Platform::macOS]
    };

    // Merge the two into result
    let mut result = Vec::new();
    for architecture in architectures {
        for platform in &platforms {
            // Generate the target triple from the architecture and platform
            result.push((*platform, architecture));
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

    let app_name = gen_app(ctx, target_triple, args, false)?;
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
        .arg(ctx.project_name.clone() + "." + target_triple + ".ipa")
        .arg("Payload")
        .status();

    if zip_cmd.is_err() || !zip_cmd.unwrap().success() {
        return Err("Error: Failed to compress the app into an IPA! Aborting.".into());
    }

    Ok(ipa_file.to_str().unwrap().to_string())
}

/// Compress everything into an .app file
fn gen_app(
    ctx: &Ctx,
    target_triple: &str,
    args: &BuildArgs,
    macos: bool,
) -> Result<String, String> {
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

    // The layout of the .app file changes between iOS and macOS, because Apple is Apple
    // See: https://developer.apple.com/library/archive/documentation/CoreFoundation/Conceptual/CFBundles/BundleTypes/BundleTypes.html
    let (new_info_plist_path, new_bin_path) = if macos {
        // The Contents folder inside the app
        let contents_path = app_path.join("Contents");
        if let Err(e) = fs::create_dir(&contents_path) {
            return Err(
                "Error: Failed to create Contents directory in the app: ".to_owned()
                    + &e.to_string(),
            );
        }

        // The MacOS folder inside the Contents path
        let macos_path = contents_path.join("MacOS");
        if let Err(e) = fs::create_dir(&macos_path) {
            return Err(
                "Error: Failed to create MacOS directory in the app: ".to_owned() + &e.to_string(),
            );
        }

        (contents_path.join("Info.plist"), macos_path.join(bin_name))
    } else {
        (app_path.join("Info.plist"), app_path.join(bin_name))
    };

    println!("   |- Copying Info.plist...");
    let info_plist = fs::read(info_plist_path);
    if info_plist.is_err() || fs::write(new_info_plist_path, info_plist.unwrap()).is_err() {
        return Err("Error: Failed to copy Info.plist to the new app".into());
    }
    println!("   |- Copying the binary...");
    let binary = fs::read(bin_path);
    if binary.is_err() || fs::write(&new_bin_path, binary.unwrap()).is_err() {
        return Err("Error: Failed to copy the binary to the new app".into());
    }
    let executable_command = Command::new("chmod")
        .arg("+x")
        .arg(new_bin_path.to_str().unwrap())
        .status();
    if executable_command.is_err() || !executable_command.unwrap().success() {
        return Err("Error: Failed to make the app's binary executable".to_string());
    }

    Ok(app_name)
}
