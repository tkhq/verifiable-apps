pub mod cli;

use std::{fs, path::PathBuf};
use qos_client::cli::{generate_file_key, advanced_provision_yubikey};
use dialoguer::{Confirm, theme::ColorfulTheme};

/// Public configuration passed in from the CLI (or tests).
#[derive(Debug, Clone)]
pub struct Config {
    pub members: usize,
    pub keys_per_member: usize,
    pub out: PathBuf,
    pub include_secrets: bool,
    pub interactive: bool,
}

pub fn run(cfg: Config) -> Result<(), Box<dyn std::error::Error>>{
    // Ensure output directory exists
    fs::create_dir_all(&cfg.out)?;

    for m in 1..=cfg.members {
        let secret_path = cfg.out.join(format!("{m}.secret"));
        let pub_path: PathBuf = cfg.out.join(format!("{m}.pub"));

        // Generate seed + pub for this member
        generate_file_key(&secret_path, &pub_path);
        println!("member {m:02}: wrote {}, {}", pub_path.display(), secret_path.display());

        // Provision configured number of yubikeys for this seed
        for k in 1..=cfg.keys_per_member {
            while !confirm_inserted_for(m, k)? {
                println!("insert YubiKey #{k} for member {m:02} and press Enter…");
            }

            loop {
                match advanced_provision_yubikey(&secret_path, None) {
                    Ok(()) => {
                        println!("provisioned yubikey {k}, member {m:02}");
                        break;
                    }
                    Err(e) => {
                        eprintln!("provisioning failed for yubikey {k}, member {m:02}, {e:?}");
                        continue;
                    }
                }
            }
        }

        if !cfg.include_secrets {
            let _ = fs::remove_file(&secret_path);
            println!("member {m:02}: removed {}", secret_path.display());
        } else {
            println!("member {m:02}: kept {}", secret_path.display());
        }
    }
    
    println!("all members provisioned");
    Ok(())
}


fn confirm_inserted_for(member: usize, k: usize) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("member {member:02} / key #{k}: Is a YubiKey inserted?"))
        .default(true)
        .show_default(true)
        .wait_for_newline(true)
        .report(true)
        .interact()?)
}
