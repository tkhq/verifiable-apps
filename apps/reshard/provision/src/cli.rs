//! CLI for reshard provisioning.

use clap::Parser;
use std::path::PathBuf;

use crate::{run, Config};

#[derive(Parser, Debug)]
#[command(
    name = "reshard_provision",
    version,
    about = "Offline Yubikey provisioning ceremony orchestrator"
)]
struct Args {
    /// Number of operators
    #[arg(long)]
    num_operators: usize,

    /// Keys per operator (default: 3)
    #[arg(long, default_value_t = 3)]
    keys_per_operator: usize,

    /// Output root
    #[arg(long)]
    out: PathBuf,

    /// Include master *.secret files in output
    #[arg(long)]
    include_secrets: bool,
}

impl Args {}

/// Provision binary command line interface.
pub struct CLI;
impl CLI {
    /// Execute the command line interface.
    pub fn execute() {
        let args = Args::parse();
        let cfg = Config {
            num_operators: args.num_operators,
            keys_per_operator: args.keys_per_operator,
            out: args.out,
            include_secrets: args.include_secrets,
        };
        if let Err(e) = run(cfg) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
