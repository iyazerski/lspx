mod cli;
mod commands;
mod daemon;
mod lsp;
mod model;
mod parse;
mod render;
mod workspace;

use anyhow::Result;
use clap::Parser;
use serde::Serialize;

use crate::cli::Cli;
use crate::model::Output;

pub fn cli_main() {
    let cli = Cli::parse();
    let error_json = cli.format.is_json();
    let result = commands::run(cli);

    match result {
        Ok(output) => {
            if let Err(error) = print_output(&output) {
                eprintln!("failed to print output: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            if error_json {
                let payload = JsonError {
                    ok: false,
                    error: error.to_string(),
                };
                let serialized = serde_json::to_string(&payload).unwrap_or_else(|_| {
                    "{\"ok\":false,\"error\":\"failed to serialize error\"}".to_string()
                });
                eprintln!("{serialized}");
            } else {
                eprintln!("error: {error}");
            }
            std::process::exit(1);
        }
    }
}

fn print_output(output: &Output) -> Result<()> {
    match output {
        Output::Json(value) => {
            println!("{}", serde_json::to_string(value)?);
        }
        Output::Text(text) => {
            println!("{text}");
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct JsonError {
    ok: bool,
    error: String,
}
