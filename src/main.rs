mod engine;
mod parser;

use std::fs;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "modmill", about = "ProTracker .mod parser and offline renderer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Parse a .mod file and print its structure.
    Parse {
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Render a .mod file to a WAV file.
    Render {
        file: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Parse { file, json } => {
            let bytes = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let module = parser::parse(&bytes)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&module)?);
            } else {
                println!("{:#?}", module);
            }
            Ok(())
        }
        Command::Render { file, out } => {
            let bytes = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
            let module = parser::parse(&bytes)?;
            engine::render_to_wav(&module, &bytes, &out)
        }
    }
}
