pub mod cli;

use std::{fs, path::PathBuf};

/// Public configuration passed in from the CLI (or tests).
#[derive(Debug, Clone)]
pub struct Config {
    pub encrypted_share_path: PathBuf,
    pub digest_path: PathBuf,
}

pub fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
