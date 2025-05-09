use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// CLI tool to run .proj scripting files
#[derive(Parser)]
#[command(name = "proj", version, about, long_about = None)]
pub struct Cli {
    /// Path to the .proj file
    #[arg(short, long, default_value = ".proj")]
    pub file: PathBuf,

    /// Activate verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Subcommands
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run a directive
    Run {
        /// Directive to run (must match a block name)
        #[arg(default_value = "main")]
        directive: String,
    },
    /// List all available directives
    List,
}
