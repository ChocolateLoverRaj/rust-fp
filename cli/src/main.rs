use std::error::Error;
use std::ops::Deref;
use clap::{Arg, Command};
use tokio::main;
use zbus::Connection;

use common::cros_fp_proxy::CrosFpProxy;

use crate::get_templates::get_templates;
use crate::set_templates::set_templates;

mod fp_file;
mod get_templates;
mod set_templates;
mod template;

#[main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cmd = Command::new("cros-fp")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("info"))
        .subcommand(Command::new("add").arg(Arg::new("label").required(true)))
        .subcommand(Command::new("list"))
        .subcommand(
            Command::new("remove")
                .arg(Arg::new("label").required(true))
                .before_help("Remove a fingerprint template"),
        )
        .subcommand(Command::new("clear").before_help("Remove all stored fingerprints for a user"))
        .subcommand(
            Command::new("match")
                .before_help("Test out the fingerprint sensor by matching a finger"),
        );
    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("info", _matches)) => {
            let connection = Connection::system().await?;
            let proxy = CrosFpProxy::new(&connection).await?;
            let info = proxy.get_fp_info().await?;
            println!("Fingerprint sensor info: {:#?}", info);
            Ok(())
        }
        Some(("add", matches)) => {
            let mut templates = get_templates().await?;
            let label = matches.get_one::<String>("label").unwrap();
            match templates.contains_key(label) {
                false => {
                    let connection = Connection::system().await?;
                    let proxy = CrosFpProxy::new(&connection).await?;
                    let operation_id = proxy.start_enroll().await?;
                    println!("Waiting for enrolling to start");
                    proxy.wait_for_operation_start(operation_id).await?;
                    // sleep(Duration::from_secs(1));
                    loop {
                        println!("Press your finger on the sensor");
                        let result = proxy.get_enroll_progress(operation_id, true).await?;
                        let progress = result.as_ref().ok_or("No enrolling data")?;
                        match progress.template.as_ref() {
                            Some(template) => {
                                templates.insert(label.to_owned(), template.to_owned());
                                set_templates(&templates).await?;
                                println!("Added template");
                                return Ok(());
                            }
                            None => {
                                // Keep going
                            }
                        }
                    }
                }
                true => Err("A fingerprint with that label already exists".into()),
            }
        }
        Some(("list", _matches)) => {
            let templates = get_templates().await?;
            println!("Fingerprints saved for this user:  {:#?}", templates.keys());
            Ok(())
        }
        Some(("remove", matches)) => {
            let mut templates = get_templates().await?;
            let label = matches.get_one::<String>("label").unwrap();
            match templates.remove(label) {
                Some(_removed_template) => {
                    println!("Removed template {:#?}", label);
                    Ok(())
                }
                None => Err(format!(
                    "Template {:#?} doesn't exist. Existing templates: {:#?}",
                    label,
                    templates.keys().collect::<Vec<_>>()
                )
                .into()),
            }
        }
        Some(("clear", _matches)) => {
            set_templates(&Default::default()).await?;
            Ok(())
        }
        Some(("match", _matches)) => {
            let connection = Connection::system().await?;
            let proxy = CrosFpProxy::new(&connection).await?;
            let templates = get_templates().await?;
            println!("Matching templates: {:#?}", templates.keys());
            if templates.len() > 0 {
                let templates = templates.iter().collect::<Vec<_>>();
                let operation_id = proxy
                    .match_finger(
                        templates
                            .iter()
                            .map(|(_label, template)| template.to_owned().to_owned())
                            .collect(),
                    )
                    .await?;
                println!("Waiting for matching to start");
                proxy.wait_for_operation_start(operation_id).await?;
                println!("Ready to match");
                let result = proxy.get_match_result(operation_id, true).await?;
                let result = postcard::from_bytes::<Option<Option<u32>>>(&result)?;
                let result = result.ok_or("No match result")?;
                match result {
                    Some(index) => {
                        println!(
                            "Matched index {:#?} ({:#?})",
                            index, templates[index as usize].0
                        );
                    }
                    None => {
                        println!("No fingerprint for this user watch matched");
                    }
                };
            } else {
                println!("No fingerprints saved for this user. Not matching.");
            }
            Ok(())
        }
        _ => unreachable!("clap should ensure we don't get here"),
    }
}
