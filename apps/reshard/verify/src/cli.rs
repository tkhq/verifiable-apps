//! CLI for reshard verification OFFLINE.

use clap::Parser;
use std::path::PathBuf;

use crate::{run, Config};

#[derive(Parser, Debug)]
#[command(
    name = "reshard_verify",
    version,
    about = "Offline share verification"
)]
struct Args {
    // Path to the encrypted share
    #[arg(long)]
    encrypted_share_path: PathBuf,

    // Path to the digest of the encrypted share (returned in the ReshardBundle)
    #[arg(long)]
    digest_path: PathBuf,
}

/// Provision binary command line interface.
pub struct CLI;
impl CLI {
    /// Execute the command line interface.
    pub fn execute() {
        let args = Args::parse();
        let cfg = Config {
            encrypted_share_path: args.encrypted_share_path,
            digest_path: args.digest_path,
        };
        
        if let Err(e) = run(cfg) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
