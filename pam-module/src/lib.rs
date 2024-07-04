#![warn(unused_crate_dependencies)]

extern crate pam;
extern crate rand;

use std::ffi::CStr;
use std::sync::mpsc::channel;
use std::thread;

use pam::constants::PamResultCode::{PAM_ABORT, PAM_AUTH_ERR, PAM_SUCCESS};
use pam::constants::{PamFlag, PamResultCode, PAM_ERROR_MSG};
use pam::conv::Conv;
use pam::module::{PamHandle, PamHooks};
use pam::pam_try;
use pollster::block_on;
use postcard::from_bytes;
use zbus::blocking::Connection;

use rust_fp::fingerprint_driver::{MatchOutput, MatchedOutput};
use rust_fp_common::get_templates::get_templates;
use rust_fp_common::rust_fp_dbus::RustFpProxyBlocking;
use rust_fp_common::set_templates::set_templates;

use crate::wait_until_unlock::wait_until_unlock;

mod wait_until_unlock;

struct RustFpPam;
pam::pam_hooks!(RustFpPam);

impl PamHooks for RustFpPam {
    // This function performs the task of authenticating the user.
    fn sm_authenticate(pamh: &mut PamHandle, _args: Vec<&CStr>, _flags: PamFlag) -> PamResultCode {
        enum Message {
            Error(String),
            Result(PamResultCode),
        }

        let (tx, rx) = channel();
        // Exit on Ctrl+C
        ctrlc::set_handler({
            let tx = tx.clone();
            move || {
                // Useful for debugging
                // Command::new("play").arg("https://www.myinstants.com/media/sounds/sudden-suspense-sound-effect.mp3").output().unwrap();
                tx.send(Message::Result(PAM_ABORT)).unwrap();
            }
        })
        .unwrap();
        // Exit if the screen was unlocked by typing the password
        thread::spawn({
            let tx = tx.clone();
            move || {
                wait_until_unlock();
                // Useful for debugging
                // Command::new("play").arg("https://www.myinstants.com/media/sounds/sudden-suspense-sound-effect.mp3").output().unwrap();
                tx.send(Message::Result(PAM_ABORT)).unwrap();
            }
        });
        // Actual fingerprint matching
        thread::spawn({
            let tx = tx.clone();
            move || {
                let authenticate = {
                    let tx = tx.clone();
                    move || -> PamResultCode {
                        let mut templates = block_on(get_templates()).unwrap();
                        if !templates.is_empty() {
                            let connection = Connection::system().unwrap();
                            let proxy = RustFpProxyBlocking::new(&connection).unwrap();
                            let templates_vec = templates.iter().collect::<Vec<_>>();
                            let max_attempts = 5;
                            for attempt in 0..max_attempts {
                                let output: MatchOutput = from_bytes(
                                    &proxy
                                        .match_templates(
                                            templates_vec
                                                .iter()
                                                .map::<Vec<u8>, _>(|(_k, v)| v.to_vec())
                                                .collect(),
                                        )
                                        .unwrap(),
                                )
                                .unwrap();
                                match output {
                                    MatchOutput::Match(MatchedOutput {
                                        index,
                                        updated_template,
                                    }) => {
                                        let matched_label = templates_vec[index].0;
                                        println!("Matched: {matched_label}.");
                                        if let Some(updated_template) = updated_template {
                                            println!(
                                                "Template was updated. Saving updated template..."
                                            );
                                            templates
                                                .insert(matched_label.to_owned(), updated_template);
                                            block_on(set_templates(&templates)).unwrap();
                                            println!("Saved updated template");
                                        }
                                        return PAM_SUCCESS;
                                    }
                                    MatchOutput::NoMatch(error) => {
                                        let remaining_attempts = max_attempts - attempt - 1;
                                        tx.send(Message::Error(format!(
                                            "No match. {remaining_attempts} attempts remaining."
                                        )))
                                        .unwrap();
                                        if let Some(error) = error {
                                            tx.send(Message::Error(format!(
                                                "Error matching: {error:?}"
                                            )))
                                            .unwrap();
                                        }
                                    }
                                }
                            }
                            PAM_AUTH_ERR
                        } else {
                            tx.send(Message::Error("No templates saved. Not matching.".into()))
                                .unwrap();
                            PAM_AUTH_ERR
                        }
                    }
                };
                tx.send(Message::Result(authenticate())).unwrap();
            }
        });
        let conv = match pamh.get_item::<Conv>() {
            Ok(Some(conv)) => conv,
            Ok(None) => todo!(),
            Err(err) => {
                println!("Couldn't get pam_conv");
                return err;
            }
        };
        for message in rx {
            match message {
                Message::Error(message) => {
                    pam_try!(conv.send(PAM_ERROR_MSG, &message));
                }
                Message::Result(result) => {
                    return result;
                }
            }
        }
        PAM_AUTH_ERR
    }
}
