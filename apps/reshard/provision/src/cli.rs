//! CLI for reshard provisioning.

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name="reshard_provision", version, about="Offline Yubikey provisioning ceremony orchestrator")]
struct Args {
    /// Number of members
    #[arg(long)]
    members: usize,

    /// Keys per member (default: 3)
    #[arg(long, default_value_t=3)]
    keys_per_member: usize,

    /// Output root (member subdirs created inside)
    #[arg(long)]
    out: PathBuf,

    /// Include master *.secret files in output 
    #[arg(long)]
    include_secrets: bool,

    /// Prompt before each key
    #[arg(long)]
    interactive: bool,
}

impl Args {}

/// Provision binary command line interface.
pub struct CLI;
impl CLI {
    /// Execute the command line interface.
    pub fn execute() {
        let args = Args::parse();
    }
}
