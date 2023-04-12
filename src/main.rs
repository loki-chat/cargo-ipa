use clap::{Parser, Subcommand};

mod build;
mod context;
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
