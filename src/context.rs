use std::{fs, path::PathBuf};
use toml::{Table, Value};

/// This is the opening portion of every Info.plist
pub const PLIST_OPENING: &str = r#"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
"#;
/// This is the closing portion of every Info.plist
pub const PLIST_CLOSING: &str = r#"
</dict>
</plist>
"#;

/// The app context
pub struct Ctx {
    /// Any configurations in the [package.metadata.cargo-ipa] section of Cargo.toml (if it exists)
    pub cfg: Option<Table>,
    /// The ID of the project, as listed in Cargo.toml
    pub project_id: String,
    /// The human-readable name of the project.
    /// This can either come from a CFBundleName setting in the [cargo-ipa]
    /// section of Cargo.toml, or can be set with the -n (or --name) argument
    pub project_name: String,
    /// The version of the project, as set in Cargo.toml
    pub project_version: String,

    /// Path to Cargo.toml
    pub cargo_toml: PathBuf,
    /// Path to target/
    pub target_dir: PathBuf,
    /// Path to the root of the project
    pub root_dir: PathBuf,
    /// Path to target/cargo-ipa
    pub cargo_ipa_dir: PathBuf,
    /// If we need to force Cargo to recompile the source code
    pub force_cargo_recompile: bool,
}
impl Ctx {
    pub fn new(name_arg: &Option<String>) -> Result<Self, String> {
        // Get all the project directories
        // Locate Cargo.toml
        let cargo_toml = match std::env::current_dir() {
            Ok(path) => match cargo::util::important_paths::find_root_manifest_for_wd(&path) {
                Ok(cfg_location) => cfg_location,
                Err(e) => return Err(format!("Failed to locate Cargo.toml: {e}")),
            },
            Err(e) => return Err(format!("Failed to get current directory: {e}")),
        };

        // Get the parent directory of Cargo.toml - the project's root directory
        let root_dir = match cargo_toml.parent() {
            Some(dir) => dir.to_owned(),
            None => return Err("Failed to get project's root directory".to_owned()),
        };

        // Try to get or create the build directories
        let target_dir = root_dir.join("target");
        if !target_dir.is_dir() {
            if let Err(e) = fs::create_dir(&target_dir) {
                return Err(format!(
                    "Failed to find or create the target directory: {e}"
                ));
            }
        }
        let cargo_ipa_dir = target_dir.join("cargo-ipa");
        if !cargo_ipa_dir.is_dir() {
            if let Err(e) = fs::create_dir(&cargo_ipa_dir) {
                return Err(format!(
                    "Failed to find or create the cargo-ipa directory: {e}"
                ));
            }
        }

        // Try to parse Cargo.toml
        let cfg_raw = match fs::read(&cargo_toml) {
            Ok(buffer) => match std::str::from_utf8(&buffer) {
                Ok(buffer_str) => match buffer_str.parse::<Table>() {
                    Ok(cfg_raw) => cfg_raw,
                    Err(e) => return Err(format!("Failed to parse Cargo.toml: {e}")),
                },
                Err(e) => return Err(format!("Failed to parse Cargo.toml: {e}")),
            },
            Err(e) => return Err(format!("Failed to read Cargo.toml: {e}")),
        };

        // These are normal Rust configurations, in the package section
        let pkg_cfg = match cfg_raw.get("package") {
            Some(pkg_cfg) => pkg_cfg,
            None => {
                return Err("Invalid Cargo.toml detected! Failed to get package name.".to_owned())
            }
        };
        let project_id = match pkg_cfg.get("name") {
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

        // These are values in the cargo-ipa section, if it exists
        let (project_name_cfg, cfg) = match pkg_cfg.get("metadata") {
            None => (None, None),
            Some(metadata_cfg) => match metadata_cfg.get("cargo-ipa") {
                None => (None, None),
                // If there is a cargo-ipa section, make sure it's valid, and also try to load the project name from it
                Some(val) => {
                    if let Value::Table(cfg) = val {
                        match cfg.get("name") {
                            Some(Value::String(name)) => {
                                (Some(name.to_owned()), Some(cfg.to_owned()))
                            }
                            _ => (None, Some(cfg.to_owned())),
                        }
                    } else {
                        println!(
                        "WARNING: Invalid `cargo-ipa` configuration format detected. Resetting to no configuration."
                    );
                        (None, None)
                    }
                }
            },
        };

        let project_name = if let Some(name) = name_arg {
            name.to_string()
        } else if let Some(name) = project_name_cfg {
            name
        } else {
            return Err("No project name could be found!".into());
        };

        Ok(Self {
            cfg,
            project_id,
            project_version,
            project_name,
            cargo_toml,
            target_dir,
            root_dir,
            cargo_ipa_dir,
            force_cargo_recompile: false,
        })
    }
}
