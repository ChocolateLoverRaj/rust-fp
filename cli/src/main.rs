use clap::{Arg, Command};
use die_exit::die;
use ectool_fp::fp_enroll::fp_enroll;
use ectool_fp::fp_get_info::fp_get_info;
use ectool_fp::fp_get_template::fp_get_template;
use ectool_fp::fp_mode::{fp_get_mode, FpModeOutput};
use ectool_fp::fp_reset::fp_reset;
use ectool_fp::fp_reset_sensor::fp_reset_sensor;
use ectool_fp::fp_seed::fp_seed;
use std::error::Error;
use std::fs::{create_dir_all, read_dir, remove_file, File, remove_dir, remove_dir_all};
use std::io::{ErrorKind, Write};

use common::get_templates_dir::get_templates_dir;
use common::seed::SEED;

fn main() -> Result<(), Box<dyn Error>> {
    let cmd = clap::Command::new("cros-fp")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("info"))
        .subcommand(Command::new("add").arg(Arg::new("user").required(true)))
        .subcommand(Command::new("list").arg(Arg::new("user").required(true)))
        .subcommand(
            Command::new("remove")
                .arg(Arg::new("user").required(true))
                .arg(Arg::new("id").required(true)),
        )
        .subcommand(
            Command::new("reset")
                .arg(Arg::new("user").required(true))
                .before_help("Remove all stored fingerprints for a user"),
        );
    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("info", _matches)) => {
            let info = fp_get_info().unwrap();
            println!("Fingerprint sensor info: {:#?}", info);
        }
        Some(("add", matches)) => {
            // FIXME: Maybe make sure the user actually exists
            let username = matches.get_one::<String>("user").unwrap();
            let fp_info = fp_get_info().unwrap();
            let templates_dir = get_templates_dir(username);
            let existing_fingerprints = read_dir(&templates_dir).map_or_else(
                |e| match e.kind() {
                    ErrorKind::NotFound => vec![],
                    _ => panic!("Error reading existing fingerprints"),
                },
                |existing_fingerprints| {
                    existing_fingerprints
                        .map(|dir_entry| dir_entry.unwrap())
                        .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_owned())
                        .collect()
                },
            );
            if existing_fingerprints.len() == fp_info.templates_slots_total {
                die!("All fingerprint slots used up for this user")
            }

            let _ = fp_seed(SEED);
            fp_reset().unwrap();
            if fp_info.templates_slots_used == fp_info.templates_slots_total {
                fp_reset_sensor().unwrap();
            }
            fp_enroll().unwrap();
            println!("Press your finger to the sensor");
            loop {
                let fp_mode = fp_get_mode().unwrap();
                if fp_mode == FpModeOutput::Reset {
                    break;
                }
                if fp_mode == FpModeOutput::Enroll {
                    fp_enroll().unwrap();
                    println!("Press your finger to the sensor")
                }
            }
            let template = fp_get_template(fp_info.templates_slots_used).unwrap();
            println!(
                "Done enrolling. Got template of size: {:#?}",
                template.len()
            );
            create_dir_all(&templates_dir).unwrap();
            let template_file_name = {
                let mut name = 0;
                loop {
                    if existing_fingerprints.contains(&name.to_string()) {
                        name += 1;
                    } else {
                        break;
                    }
                }
                name.to_string()
            };
            let mut template_file =
                File::create(format!("{}/{}", &templates_dir, template_file_name)).unwrap();
            template_file.write_all(&template).unwrap();
        }
        Some(("list", matches)) => {
            let username = matches.get_one::<String>("user").unwrap();
            let templates_dir = get_templates_dir(username);
            let existing_fingerprints = read_dir(&templates_dir).map_or(vec![], |dir| {
                dir.map(|dir_entry| dir_entry.unwrap().file_name().to_str().unwrap().to_owned())
                    .collect()
            });
            println!(
                "Fingerprints saved for the user {:#?}: {:#?}",
                username, existing_fingerprints,
            );
        }
        Some(("remove", matches)) => {
            let username = matches.get_one::<String>("user").unwrap();
            let id = matches.get_one::<String>("id").unwrap();
            match remove_file(format!("{}/{}", get_templates_dir(username), id)) {
                Ok(_) => {
                    println!("Removed fingerprint");
                }
                Err(e) => match e.kind() {
                    ErrorKind::NotFound => {
                        println!("That fingerprint doesn't exist");
                    }
                    _ => {
                        return Err(Box::new(e));
                    }
                },
            }
        },
        Some(("reset", matches)) => {
            let username = matches.get_one::<String>("user").unwrap();
            match remove_dir_all(get_templates_dir(username)) {
                Ok(()) => {},
                Err(e) => {
                    match e.kind() {
                        ErrorKind::NotFound => {},
                        _ => return Err(Box::new(e))
                    }
                }
            }
        },
        _ => unreachable!("clap should ensure we don't get here"),
    };
    Ok(())
}
