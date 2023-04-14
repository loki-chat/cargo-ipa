use clap::{Parser, Subcommand};

mod build;
mod context;
mod sign;
use context::*;

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
    /// Sign an IPA
    Sign,
}

/// Prints an error, with bold & red text
fn error(msg: String) {
    println!("\x1B[1;31m{msg}\x1B[0m");
}

#[cfg(target_os = "macos")]
fn main() {
    // Run the clap app & get the provided command
    let Cli::Ipa(cmd) = Cli::parse();

    // Match the command & run code accordingly
    match cmd {
        Commands::Build(args) => {
            if let Err(e) = build::build(args) {
                error(e);
            }
        }
        Commands::Sign => {
            if let Err(e) = sign::sign() {
                error(e);
            }
        }
    };
}

#[cfg(not(target_os = "macos"))]
fn main() {
    panic!("Only macOS is supported by cargo-ipa. iOS apps can't be compiled on non-mac systems.");
}
