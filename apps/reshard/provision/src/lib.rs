pub mod cli;

use dialoguer::{theme::ColorfulTheme, Confirm};
use qos_client::cli::{advanced_provision_yubikey, generate_file_key};
use std::{fs, path::PathBuf};
use tempdir::TempDir;

/// Public configuration passed in from the CLI (or tests).
#[derive(Debug, Clone)]
pub struct Config {
    pub members: usize,
    pub keys_per_member: usize,
    pub out: PathBuf,
    pub include_secrets: bool,
}

pub fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure output directory exists
    fs::create_dir_all(&cfg.out)?;

    for m in 1..=cfg.members {
        let tmp_dir = TempDir::new("secrets").unwrap();
        let tmp_secret_path = tmp_dir.path().join(format!("{m}.secret"));
        let pub_path: PathBuf = cfg.out.join(format!("{m}.pub"));

        // Generate seed + pub for this member
        generate_file_key(&tmp_secret_path, &pub_path);

        // Provision configured number of yubikeys for this seed
        for k in 1..=cfg.keys_per_member {
            let prompt = format!("please insert yubikey {k} for member {m}. are you ready?");
            while !confirm_yes(&prompt, false)? {
                println!("oops that wasn't correct. have you recently 420'd?");
            }

            loop {
                match advanced_provision_yubikey(&tmp_secret_path, None) {
                    Ok(()) => {
                        println!("provisioned yubikey {k}, member {m}");
                        break;
                    }
                    Err(e) => {
                        eprintln!("provisioning failed for yubikey {k}, member {m}, {e:?}");
                        continue;
                    }
                }
            }
        }

        if cfg.include_secrets {
            let secret_path = cfg.out.join(format!("{m}.secret"));
            fs::copy(&tmp_secret_path, &secret_path)?;
            println!("kept {}", secret_path.display())
        } else {
            println!("secret for member {m} stayed in tmp/secrets and was removed)");
        }

        // tmp_dir drops out of scope here and is therefore removed
    }

    println!("all members provisioned");
    Ok(())
}

fn confirm_yes(prompt: &str, default_yes: bool) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt.to_string())
        .default(default_yes)
        .show_default(true)
        .wait_for_newline(true)
        .report(false)
        .interact()?)
}
