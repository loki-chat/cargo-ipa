use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};
use toml::{Table, Value};

mod build;
mod consts;
use consts::*;

// The CLI application
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
enum Cli {
    #[command(subcommand)]
    Ipa(Commands),
}

#[derive(Subcommand)]
enum Commands {
    /// Compile a Rust binary or library example into an IPA.
    Build(build::BuildArgs),
}

// The app context
pub struct Ctx {
    build_dir: PathBuf,
    bin_dir: PathBuf,
    cfg: Option<Table>,
    project_name: String,
    project_version: String,
}
impl Ctx {
    pub fn new() -> Result<Self, String> {
        // Locate Cargo.toml
        let cargo_toml_location = match std::env::current_dir() {
            Ok(path) => match cargo::util::important_paths::find_root_manifest_for_wd(&path) {
                Ok(cfg_location) => cfg_location,
                Err(e) => return Err(format!("Failed to locate Cargo.toml: {e}")),
            },
            Err(e) => return Err(format!("Failed to get current directory: {e}")),
        };

        // Get the parent directory of Cargo.toml - the project's root directory
        let root_dir = match cargo_toml_location.parent() {
            Some(dir) => dir.to_owned(),
            None => return Err("Failed to get project's root directory!".to_owned()),
        };

        // Try to parse Cargo.toml
        let cfg_raw = match fs::read(&cargo_toml_location) {
            Ok(buffer) => match std::str::from_utf8(&buffer) {
                Ok(buffer_str) => match buffer_str.parse::<Table>() {
                    Ok(cfg_raw) => cfg_raw,
                    Err(e) => return Err(format!("Failed to parse Cargo.toml: {e}")),
                },
                Err(e) => return Err(format!("Failed to parse Cargo.toml: {e}")),
            },
            Err(e) => return Err(format!("Failed to read Cargo.toml: {e}")),
        };
        let pkg_cfg = match cfg_raw.get("package") {
            Some(pkg_cfg) => pkg_cfg,
            None => {
                return Err("Invalid Cargo.toml detected! Failed to get package name.".to_owned())
            }
        };
        let project_name = match pkg_cfg.get("name") {
            Some(Value::String(name)) => name.to_owned(),
            _ => return Err("Invalid Cargo.toml detected! Failed to get package name.".to_owned()),
        };
        let project_version = match pkg_cfg.get("version") {
            Some(Value::String(version)) => version.to_owned(),
            _ => {
                return Err(
                    "Invalid Cargo.toml detected! Failed to get package version.".to_owned(),
                )
            }
        };
        let cfg = match cfg_raw.get("cargo-ipa") {
            None => None,
            Some(val) => {
                if let Value::Table(cfg) = val {
                    Some(cfg.to_owned())
                } else {
                    println!(
                        "WARNING: Invalid `cargo-ipa` configuration format detected. Defaulting to None."
                    );
                    None
                }
            }
        };

        // Try to get or create the build directories
        let build_dir = root_dir.join("target");
        if !build_dir.is_dir() {
            if let Err(e) = fs::create_dir(&build_dir) {
                return Err(format!("Failed to create build directory: {e}"));
            }
        }
        let bin_dir = build_dir.join(TARGET);
        if !build_dir.is_dir() {
            if let Err(e) = fs::create_dir(&bin_dir) {
                return Err(format!("Failed to create build directory: {e}"));
            }
        }
        let build_dir = build_dir.join("Payload");
        if build_dir.is_dir() {
            if let Err(e) = fs::remove_dir_all(&build_dir) {
                return Err(format!("Failed to clean old build files: {e}"));
            }
        }
        if let Err(e) = fs::create_dir(&build_dir) {
            return Err(format!("Failed to create build directory: {e}"));
        }

        Ok(Self {
            build_dir,
            bin_dir,
            cfg,
            project_name,
            project_version,
        })
    }
}

fn main() {
    // Run the clap app & get the provided command
    let Cli::Ipa(cmd) = Cli::parse();

    // Make the app context
    let ctx = Ctx::new().unwrap();

    // Match the command & run code accordingly
    match cmd {
        Commands::Build(args) => build::build(args, &ctx),
    }
}
