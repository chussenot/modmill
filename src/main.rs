mod engine;
mod parser;

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "modmill",
    about = "ProTracker .mod parser and offline renderer",
    long_about = "ProTracker .mod parser and offline renderer.\n\n\
Parses the classic 4-channel \"M.K.\" .mod format and can either dump its \
structure (header, samples, patterns) or render it to a standalone WAV file."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a .mod file and print its structure.
    Parse {
        /// Path to the .mod file to parse.
        file: PathBuf,
        /// Print the parsed module as JSON instead of Rust debug output.
        #[arg(long)]
        json: bool,
    },
    /// Render a .mod file to a WAV file.
    Render {
        /// Path to the .mod file to render.
        file: PathBuf,
        /// Path to write the rendered WAV file to.
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file, json } => {
            let bytes = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let module = parser::parse(&bytes)
                .with_context(|| format!("parsing {} as a .mod file", file.display()))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&module)?);
            } else {
                println!("{:#?}", module);
            }
            Ok(())
        }
        Command::Render { file, out } => {
            let bytes = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let module = parser::parse(&bytes)
                .with_context(|| format!("parsing {} as a .mod file", file.display()))?;
            engine::render_to_wav(&module, &bytes, &out)
        }
    }
}
