pub mod cli;

use dialoguer::{theme::ColorfulTheme, Confirm};
use qos_client::cli::{advanced_provision_yubikey, generate_file_key};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A temporary directory that is removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(prefix: &str) -> std::io::Result<Self> {
        let name = format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = PathBuf::from("/tmp").join(name);
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Public configuration passed in from the CLI (or tests).
#[derive(Debug, Clone)]
pub struct Config {
    pub num_operators: usize,
    pub keys_per_operator: usize,
    pub out: PathBuf,
    pub include_secrets: bool,
}

pub fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    println!("YubiKey provisioning is about to start. This is serious.");
    if confirm_yes("Are you inebriated?", true)? {
        eprintln!("Aborting provisioning — please try again when sober.");
        return Err("operator indicated inebriation".into());
    }

    // Ensure output directory exists
    fs::create_dir_all(&cfg.out)?;

    for m in 1..=cfg.num_operators {
        let pub_path: PathBuf = cfg.out.join(format!("{m}.pub"));
        if pub_path.exists() {
            let skip_prompt =
                format!("Found existing public key for operator {m}. Skip provisioning?");
            let skip = confirm_yes(&skip_prompt, true)?;

            if skip {
                println!("Skipping operator {m}");
                continue;
            }
        }

        let tmp_dir = TempDir::new("reshard-secrets").expect("create temp dir");
        let tmp_secret_path = tmp_dir.path().join(format!("{m}.secret"));

        generate_file_key(&tmp_secret_path, &pub_path);

        // Provision configured number of yubikeys for this seed
        for k in 1..=cfg.keys_per_operator {
            let prompt = format!("Please insert yubikey {k} for operator {m}. Are you ready?");
            while !confirm_yes(&prompt, false)? {
                println!("Oops that wasn't correct.");
            }

            loop {
                match advanced_provision_yubikey(&tmp_secret_path, None) {
                    Ok(()) => {
                        println!("Provisioned yubikey {k}, operator {m}");
                        break;
                    }
                    Err(e) => {
                        eprintln!("provisioning failed for yubikey {k}, operator {m}, {e:?}");
                        continue;
                    }
                }
            }
        }

        if cfg.include_secrets {
            let secret_path = cfg.out.join(format!("{m}.secret"));
            fs::copy(&tmp_secret_path, &secret_path)?;
            println!("Kept {}", secret_path.display())
        } else {
            println!("Secret for operator {m} stayed in tmp/secrets and was removed)");
        }

        drop(tmp_dir) // tmp_dir drops out of scope here and is therefore removed but making it explicit
    }

    println!("All operator yubikeys provisioned!");
    Ok(())
}

fn confirm_yes(prompt: &str, default_yes: bool) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(default_yes)
        .show_default(true)
        .wait_for_newline(true)
        .report(false)
        .interact()?)
}
